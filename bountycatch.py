import argparse
import redis
import os
import json
import logging
import re
from datetime import datetime
from typing import Optional, Set, Dict, Any
from pathlib import Path

class DataStore:
    def __init__(self, host='localhost', port=6379, db=0, max_connections=10):
        self.pool = redis.ConnectionPool(
            host=host, port=port, db=db, max_connections=max_connections
        )
        self.r = redis.Redis(connection_pool=self.pool)
        self.logger = logging.getLogger(__name__)
        
        try:
            self.r.ping()
            self.logger.info(f"Connected to Redis at {host}:{port}")
        except redis.ConnectionError as e:
            self.logger.error(f"Failed to connect to Redis: {e}")
            raise

    def add_domain(self, domain: str) -> int:
        try:
            return self.r.sadd('domains', domain)
        except redis.RedisError as e:
            self.logger.error(f"Failed to add domain {domain}: {e}")
            raise

    def remove_domain(self, domain: str) -> int:
        """Remove a domain from the database"""
        try:
            return self.r.srem('domains', domain)
        except redis.RedisError as e:
            self.logger.error(f"Failed to remove domain {domain}: {e}")
            raise

    def get_domains(self) -> Set[bytes]:
        try:
            return self.r.smembers('domains')
        except redis.RedisError as e:
            self.logger.error(f"Failed to get domains: {e}")
            raise

    def deduplicate(self):
        return True

    def delete_all_domains(self) -> int:
        try:
            return self.r.delete('domains')
        except redis.RedisError as e:
            self.logger.error(f"Failed to delete all domains: {e}")
            raise
    
    def domains_exist(self) -> bool:
        try:
            return bool(self.r.exists('domains'))
        except redis.RedisError as e:
            self.logger.error(f"Failed to check if domains exist: {e}")
            raise

    def count_domains(self) -> int:
        try:
            return self.r.scard('domains')
        except redis.RedisError as e:
            self.logger.error(f"Failed to count domains: {e}")
            raise
            
class DomainValidator:
    
    # Single comprehensive regex pattern that handles all valid domain formats:
    # - Leading wildcards: *.example.com
    # - Internal wildcards: svc-*.domain.com, rac-*.net.dell.com, test.*.invalid.com
    # - Service records: _service.domain.com, _collab-edge.5g.dell.com
    # - Standard domains: example.com, sub.domain.com
    DOMAIN_PATTERN = re.compile(
        r'^(?:'
        r'(?:\*\.)?'  # Optional leading wildcard: *.
        r'(?:[a-zA-Z0-9_*](?:[a-zA-Z0-9_*-]{0,61}[a-zA-Z0-9_*])?\.)'  # Labels (allows * anywhere in label)
        r'+'  # One or more labels
        r'[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?'  # TLD (no * or _ allowed)
        r')$'
    )
    
    @classmethod
    def is_valid_domain(cls, domain: str) -> bool:
        if not domain or len(domain) > 253:
            return False
        
        # Check for invalid patterns that the regex might miss
        if domain.startswith('*') and not domain.startswith('*.'):
            return False  # *abc.com is invalid
        
        if domain.endswith('*') or domain == '*':
            return False  # svc-* without TLD is invalid
        
        if '.-' in domain or '-.' in domain or domain.startswith('.') or domain.endswith('.'):
            return False  # -.example.com and similar invalid patterns
        
        return bool(cls.DOMAIN_PATTERN.match(domain))

class ConfigManager:
    
    def __init__(self, config_file: Optional[str] = None):
        self.config = self._load_config(config_file)
        self.logger = logging.getLogger(__name__)
    
    def _load_config(self, config_file: Optional[str]) -> Dict[str, Any]:
        default_config = {
            'redis': {
                'host': 'localhost',
                'port': 6379,
                'db': 0,
                'max_connections': 10
            },
            'logging': {
                'level': 'INFO',
                'format': '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
            }
        }
        
        if config_file and Path(config_file).exists():
            try:
                with open(config_file, 'r') as f:
                    file_config = json.load(f)
                    for section, values in file_config.items():
                        if section in default_config:
                            default_config[section].update(values)
                        else:
                            default_config[section] = values
            except (json.JSONDecodeError, IOError) as e:
                logging.warning(f"Failed to load config file {config_file}: {e}")
        
        redis_host = os.getenv('REDIS_HOST')
        if redis_host:
            default_config['redis']['host'] = redis_host
        
        redis_port = os.getenv('REDIS_PORT')
        if redis_port:
            try:
                default_config['redis']['port'] = int(redis_port)
            except ValueError:
                logging.warning(f"Invalid REDIS_PORT value: {redis_port}")
        
        return default_config
    
    def get_redis_config(self) -> Dict[str, Any]:
        return self.config['redis']
    
    def get_logging_config(self) -> Dict[str, Any]:
        return self.config['logging']

class DomainManager:
    def __init__(self, datastore):
        self.datastore = datastore
        self.logger = logging.getLogger(__name__)

    def iter_domains(self, match: Optional[str] = None, regex: Optional[re.Pattern] = None, sort: bool = False, batch_size: int = 500000):
        """Stream domains from Redis using SSCAN to reduce latency and memory use.

        If sort is False (default), yields in scan order (faster, low memory).
        If sort is True, collects then sorts before yielding (slower, higher memory).
        """
        if sort:
            # Fall back to existing get_domains + sort when explicit sort is requested
            domains = self.get_domains()
            if match:
                domains = {d for d in domains if match in d}
            if regex:
                domains = {d for d in domains if regex.search(d)}
            for d in sorted(domains):
                yield d
            return

        cursor = 0
        while True:
            cursor, batch = self.datastore.r.sscan('domains', cursor=cursor, count=batch_size)
            if not batch:
                if cursor == 0:
                    break
            for raw in batch:
                d = raw.decode('utf-8')
                if match and match not in d:
                    continue
                if regex and not regex.search(d):
                    continue
                yield d
            if cursor == 0:
                break
    
    def export_domains(self, output_file: str, format_type: str = 'text') -> bool:
        try:
            domains = self.get_domains()
            if not domains:
                self.logger.warning("No domains found in database")
                return False
            
            output_path = Path(output_file)
            
            if format_type.lower() == 'json':
                export_data = {
                    'domain_count': len(domains),
                    'exported_at': str(datetime.now().isoformat()),
                    'domains': sorted(list(domains))
                }
                
                with open(output_path, 'w') as f:
                    json.dump(export_data, f, indent=2)
                    
            elif format_type.lower() == 'text':
                with open(output_path, 'w') as f:
                    for domain in sorted(domains):
                        f.write(f"{domain}\n")
            else:
                self.logger.error(f"Unsupported export format: {format_type}")
                return False
            
            self.logger.info(f"Exported {len(domains)} domains to {output_file} ({format_type} format)")
            return True
            
        except (IOError, json.JSONEncodeError) as e:
            self.logger.error(f"Failed to export domains: {e}")
            return False

    def _process_domain(self, domain: str) -> str:
        """Process domain to handle special cases while keeping them valid for storage."""
        # Remove any leading/trailing whitespace
        domain = domain.strip()
        
        # Domain is stored as-is since our validator now accepts these patterns
        # No sanitization needed - we want to preserve the original format
        return domain

    def add_domains_from_file(self, filename: str, validate: bool = True, batch_size: int = 500000) -> None:
        file_path = Path(filename)
        if not file_path.exists():
            self.logger.error(f"File {filename} does not exist")
            return
        
        try:
            pipeline = self.datastore.r.pipeline(transaction=False)
            batch: list[str] = []
            with open(file_path, 'r') as file:
                total_domains = 0
                new_domains = 0
                invalid_domains = 0
                
                for line_num, line in enumerate(file, 1):
                    domain = line.strip()
                    if not domain:
                        continue
                    
                    processed_domain = self._process_domain(domain)
                    
                    if validate and not DomainValidator.is_valid_domain(processed_domain):
                        self.logger.warning(f"Invalid domain '{domain}' on line {line_num}, skipping")
                        invalid_domains += 1
                        continue
                    
                    batch.append(processed_domain)
                    total_domains += 1

                    if len(batch) >= batch_size:
                        try:
                            added = pipeline.sadd('domains', *batch)
                            pipeline_results = pipeline.execute()
                            # pipeline_results is a list; we expect a single int for SADD
                            if pipeline_results:
                                new_domains += pipeline_results[-1]
                        except redis.RedisError as e:
                            self.logger.error(f"Failed to add batch: {e}")
                        batch.clear()
            # Flush remaining batch
            if batch:
                try:
                    added = pipeline.sadd('domains', *batch)
                    pipeline_results = pipeline.execute()
                    if pipeline_results:
                        new_domains += pipeline_results[-1]
                except redis.RedisError as e:
                    self.logger.error(f"Failed to add final batch: {e}")
                batch.clear()
                
                duplicate_domains = total_domains - new_domains
                duplicate_percentage = (duplicate_domains / total_domains * 100) if total_domains > 0 else 0
                
                self.logger.info(
                    f"Processed {total_domains} domains: {new_domains} new, "
                    f"{duplicate_domains} duplicates ({duplicate_percentage:.2f}%)"
                )
                if invalid_domains > 0:
                    self.logger.warning(f"Skipped {invalid_domains} invalid domains")
                    
        except IOError as e:
            self.logger.error(f"Failed to read file {filename}: {e}")

    def remove_domains_from_file(self, filename: str) -> None:
        """Remove domains listed in a file from the database"""
        file_path = Path(filename)
        if not file_path.exists():
            self.logger.error(f"File {filename} does not exist")
            return
        
        try:
            with open(file_path, 'r') as file:
                total_domains = 0
                removed_domains = 0
                not_found_domains = 0
                
                for line_num, line in enumerate(file, 1):
                    domain = line.strip()
                    if not domain:
                        continue
                    
                    try:
                        removed = self.datastore.remove_domain(domain)
                        if removed > 0:
                            removed_domains += 1
                        else:
                            not_found_domains += 1
                            self.logger.warning(f"Domain '{domain}' not found in database")
                        total_domains += 1
                    except redis.RedisError as e:
                        self.logger.error(f"Failed to remove domain '{domain}': {e}")
                
                self.logger.info(
                    f"Processed {total_domains} domains: {removed_domains} removed, "
                    f"{not_found_domains} not found"
                )
                    
        except IOError as e:
            self.logger.error(f"Failed to read file {filename}: {e}")

    def remove_domain(self, domain: str) -> bool:
        """Remove a single domain from the database"""
        try:
            removed = self.datastore.remove_domain(domain)
            if removed > 0:
                self.logger.info(f"Removed domain '{domain}' from database")
                return True
            else:
                self.logger.warning(f"Domain '{domain}' not found in database")
                return False
        except redis.RedisError as e:
            self.logger.error(f"Failed to remove domain '{domain}': {e}")
            return False

    def get_domains(self) -> Set[str]:
        try:
            raw_domains = self.datastore.get_domains()
            return {domain.decode('utf-8') for domain in raw_domains}
        except redis.RedisError as e:
            self.logger.error(f"Failed to get domains: {e}")
            return set()
    
    def count_domains(self) -> Optional[int]:
        try:
            if not self.datastore.domains_exist():
                self.logger.error("No domains exist in database")
                return None
            
            count = self.datastore.count_domains()
            self.logger.info(f"Database contains {count} domains")
            return count
        except redis.RedisError as e:
            self.logger.error(f"Failed to count domains: {e}")
            return None

    def deduplicate(self) -> bool:
        return self.datastore.deduplicate()
    
    def delete_all(self) -> bool:
        try:
            self.logger.info("Attempting to delete all domains")
            deleted_count = self.datastore.delete_all_domains()
            
            if deleted_count == 0:
                self.logger.warning("No domains existed in database")
                return False
            else:
                self.logger.info("All domains deleted successfully")
                return True
        except redis.RedisError as e:
            self.logger.error(f"Failed to delete all domains: {e}")
            return False

def setup_logging(config: ConfigManager, silent: bool = False) -> None:
    logging_config = config.get_logging_config()
    handlers = [logging.FileHandler('bountycatch.log')]
    if not silent:
        handlers.insert(0, logging.StreamHandler())

    logging.basicConfig(
        level=getattr(logging, logging_config['level']),
        format=logging_config['format'],
        handlers=handlers
    )

def main():
    parser = argparse.ArgumentParser(
        description="Manage bug bounty targets",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    add    : %(prog)s add -f domains.txt
             %(prog)s add -f raw.txt --no-validate
    export : %(prog)s export -f out.json --format json --match .dell.com
    count  : %(prog)s count --regex '.*\\.dell\\.com$'
    print  : %(prog)s print --match .dell.com --sort
    remove : %(prog)s remove --match .dell.com
    delete : %(prog)s delete-all
    silent : %(prog)s print --match .dell.com --silent

Filter flags (where supported):
    --match SUBSTR   substring filter
    --regex PATTERN  regex filter
    --sort           sorted output (print/export only; slower)
        """
    )
    
    subparsers = parser.add_subparsers(dest='command', help='Available commands')
    
    add_parser = subparsers.add_parser('add', help='Add domains from file')
    add_parser.add_argument('-f', '--file', required=True, help='File containing domains')
    add_parser.add_argument('--no-validate', action='store_true', help='Skip domain validation')
    
    export_parser = subparsers.add_parser('export', help='Export domains to file (supports filtering)')
    export_parser.add_argument('-f', '--file', required=True, help='Output file')
    export_parser.add_argument('--format', choices=['text', 'json'], default='text', help='Export format')
    export_filter = export_parser.add_mutually_exclusive_group()
    export_filter.add_argument('--match', help='Filter domains containing this substring')
    export_filter.add_argument('--regex', help='Filter domains matching this regex')
    export_parser.add_argument('--sort', action='store_true', help='Sort domains before exporting (slower)')
    
    print_parser = subparsers.add_parser('print', help='Print domains (supports filtering)')
    print_filter = print_parser.add_mutually_exclusive_group()
    print_filter.add_argument('--match', help='Filter domains containing this substring')
    print_filter.add_argument('--regex', help='Filter domains matching this regex')
    print_parser.add_argument('--sort', action='store_true', help='Sort domains before printing (slower)')
    
    count_parser = subparsers.add_parser('count', help='Count domains in database (supports filtering)')
    count_filter = count_parser.add_mutually_exclusive_group()
    count_filter.add_argument('--match', help='Filter domains containing this substring before counting')
    count_filter.add_argument('--regex', help='Filter domains matching this regex before counting')
    
    remove_parser = subparsers.add_parser('remove', help='Remove domains from database (supports filtering)')
    remove_group = remove_parser.add_mutually_exclusive_group(required=True)
    remove_group.add_argument('-f', '--file', help='File containing domains to remove')
    remove_group.add_argument('-d', '--domain', help='Single domain to remove')
    remove_group.add_argument('--match', help='Remove domains containing this substring')
    remove_group.add_argument('--regex', help='Remove domains matching this regex')
    
    delete_parser = subparsers.add_parser('delete-all', help='Delete all domains')
    delete_parser.add_argument('--confirm', action='store_true', help='Skip confirmation prompt')
    
    parser.add_argument('-c', '--config', help='Configuration file path')
    parser.add_argument('-v', '--verbose', action='store_true', help='Enable verbose logging')
    parser.add_argument('-s', '--silent', action='store_true', help='Suppress console logs; only emit command output')
    
    args = parser.parse_args()
    
    if not args.command:
        parser.print_help()
        return
    
    config = ConfigManager(args.config)
    
    if args.verbose:
        config.config['logging']['level'] = 'DEBUG'
    
    setup_logging(config, silent=args.silent)
    logger = logging.getLogger(__name__)

    def compile_regex(pattern_str: Optional[str]):
        if not pattern_str:
            return None
        try:
            return re.compile(pattern_str)
        except re.error as e:
            logger.error(f"Invalid regex '{pattern_str}': {e}")
            return None
    
    try:
        redis_config = config.get_redis_config()
        datastore = DataStore(**redis_config)
        domain_manager = DomainManager(datastore)
        
    except redis.ConnectionError:
        logger.error("Failed to connect to Redis. Please check your Redis server is running.")
        return 1
    except Exception as e:
        logger.error(f"Initialisation error: {e}")
        return 1

    if args.command == 'add':
        validate = not args.no_validate
        domain_manager.add_domains_from_file(args.file, validate=validate)
        domain_manager.deduplicate()
        
    elif args.command == 'export':
        pattern = compile_regex(args.regex)
        if args.regex and pattern is None:
            return 1

        count_written = 0
        try:
            if args.format == 'json':
                domains = list(domain_manager.iter_domains(match=args.match, regex=pattern, sort=args.sort))
                export_data = {
                    'domain_count': len(domains),
                    'exported_at': str(datetime.now().isoformat()),
                    'domains': domains if not args.sort else sorted(domains)
                }
                with open(args.file, 'w') as f:
                    json.dump(export_data, f, indent=2)
                count_written = export_data['domain_count']
            else:
                # text export, stream to file to reduce memory
                with open(args.file, 'w') as f:
                    domains_iter = domain_manager.iter_domains(match=args.match, regex=pattern, sort=args.sort)
                    for d in domains_iter:
                        f.write(f"{d}\n")
                        count_written += 1
            logger.info(f"Exported {count_written} domains to {args.file} ({args.format} format)")
        except (IOError, json.JSONEncodeError) as e:
            logger.error(f"Failed to export domains: {e}")
            return 1
            
    elif args.command == 'print':
        pattern = compile_regex(args.regex)
        if args.regex and pattern is None:
            return 1

        # Stream via SSCAN to reduce startup latency and memory usage
        found_any = False
        try:
            for domain in domain_manager.iter_domains(match=args.match, regex=pattern, sort=args.sort):
                found_any = True
                print(domain)
        except BrokenPipeError:
            # Handle broken pipe gracefully when piping to head, etc.
            pass

        if not found_any:
            logger.warning("No domains found in database")
                
    elif args.command == 'count':
        pattern = compile_regex(args.regex)
        if args.regex and pattern is None:
            return 1

        total = 0
        try:
            for _ in domain_manager.iter_domains(match=args.match, regex=pattern, sort=False):
                total += 1
            print(f"{total}")
        except redis.RedisError as e:
            logger.error(f"Failed to count domains: {e}")
            return 1
    
    elif args.command == 'remove':
        if args.file:
            domain_manager.remove_domains_from_file(args.file)
        elif args.domain:
            if domain_manager.remove_domain(args.domain):
                print(f"Domain '{args.domain}' removed from database")
            else:
                return 1
        elif args.match or args.regex:
            pattern = compile_regex(args.regex)
            if args.regex and pattern is None:
                return 1
            removed = 0
            try:
                # Collect candidates then remove to avoid modifying set during scan
                to_remove = list(domain_manager.iter_domains(match=args.match, regex=pattern, sort=False))
                for d in to_remove:
                    removed += domain_manager.datastore.remove_domain(d)
                logger.info(f"Removed {removed} domains using filter")
            except redis.RedisError as e:
                logger.error(f"Failed to remove domains with filter: {e}")
                return 1
        else:
            return 1
            
    elif args.command == 'delete-all':
        if not args.confirm:
            response = input("Are you sure you want to delete ALL domains from the database? (y/N): ")
            if response.lower() not in ['y', 'yes']:
                logger.info("Delete operation cancelled")
                return 0
        
        if domain_manager.delete_all():
            print("All domains deleted successfully")
        else:
            return 1
    
    return 0

if __name__ == '__main__':
    import sys
    try:
        exit_code = main()
        sys.exit(exit_code or 0)
    except KeyboardInterrupt:
        print("\nOperation cancelled by user")
        sys.exit(1)
    except BrokenPipeError:
        # Handle broken pipe gracefully (e.g., when piping to head)
        sys.exit(0)
    except Exception as e:
        logging.error(f"Unexpected error: {e}")
        sys.exit(1)
