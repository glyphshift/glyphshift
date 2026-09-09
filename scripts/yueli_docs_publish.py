"""Portable Docs publisher. Python standard library only; never prints credentials."""
import argparse
import ipaddress
import json
import os
from pathlib import Path
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zipfile

MAX_ZIP = 100 * 1024 * 1024
EXTENSIONS = {'.md', '.png', '.jpg', '.jpeg', '.webp', '.gif'}


def pack(root, output):
    root, output = Path(root).resolve(), Path(output).resolve()
    if not root.is_dir():
        raise ValueError('Documentation directory does not exist')
    entries = []
    total = 0
    for entry in sorted(root.rglob('*')):
        relative = entry.relative_to(root)
        if any(part.startswith('.') or part in {'node_modules', '__pycache__'} for part in relative.parts):
            continue
        if entry.is_symlink():
            raise ValueError('Symlinks are not supported: ' + relative.as_posix())
        if not entry.is_file() or (entry.suffix.lower() not in EXTENSIONS and relative.as_posix() != 'docs.json'):
            continue
        if not entry.resolve().is_relative_to(root):
            raise ValueError('File is outside documentation directory')
        size = entry.stat().st_size
        if size > 25 * 1024 * 1024:
            raise ValueError('File exceeds 25 MiB: ' + relative.as_posix())
        total += size
        entries.append((relative.as_posix(), entry))
    if not any(name.endswith('.md') for name, _ in entries):
        raise ValueError('No Markdown files found')
    if len(entries) > 20000 or total > 1024**3:
        raise ValueError('Documentation exceeds package limits')
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + '.tmp-' + uuid.uuid4().hex)
    try:
        with zipfile.ZipFile(temporary, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
            for name, entry in entries:
                info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, entry.read_bytes())
        if temporary.stat().st_size > MAX_ZIP:
            raise ValueError('ZIP exceeds 100 MiB')
        temporary.replace(output)
    finally:
        temporary.unlink(missing_ok=True)
    return {'package': str(output), 'files': len(entries), 'bytes': output.stat().st_size}


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('API redirect refused; supply the canonical Docs origin')


class Client:
    def __init__(self, base):
        parsed = urllib.parse.urlsplit(base)
        try:
            local = ipaddress.ip_address(parsed.hostname or '').is_loopback
        except ValueError:
            local = (parsed.hostname or '') == 'localhost' or (parsed.hostname or '').endswith('.dev.yuelili.test')
        if parsed.scheme != 'https' and not (parsed.scheme == 'http' and local):
            raise ValueError('HTTPS is required except for local development')
        if not parsed.netloc or parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.path not in {'', '/'}:
            raise ValueError('--base must be a Docs origin without path or credentials')
        self.base = base.rstrip('/')
        self.token = os.environ.get('YUELI_DOCS_TOKEN', '').strip()
        if not self.token.startswith('pat_'):
            raise ValueError('Set YUELI_DOCS_TOKEN to a scoped developer token')
        self.opener = urllib.request.build_opener(NoRedirect())

    def request(self, path, data=None, content_type='application/json', timeout=1800):
        request = urllib.request.Request(self.base + '/api/v1/imports/docs' + path, data=data,
            headers={'Authorization': 'Bearer ' + self.token, 'Content-Type': content_type})
        try:
            with self.opener.open(request, timeout=timeout) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            # Server output may contain caller-provided data. Do not dump it or headers.
            raise ValueError('Docs HTTP error ' + str(error.code)) from None
        except (TimeoutError, urllib.error.URLError):
            raise ValueError('Request interrupted; query the known batch ID before retrying') from None

    def upload(self, archive, collection, locale, mode):
        file = Path(archive)
        if file.stat().st_size > MAX_ZIP:
            raise ValueError('ZIP exceeds 100 MiB')
        boundary = 'docs-' + uuid.uuid4().hex
        pieces = []
        for key, value in {'collection': collection, 'defaultLocale': locale, 'mode': mode}.items():
            pieces.append(('--' + boundary + '\r\nContent-Disposition: form-data; name="' + key + '"\r\n\r\n' + value + '\r\n').encode())
        pieces.extend([('--' + boundary + '\r\nContent-Disposition: form-data; name="file"; filename="docs.zip"\r\nContent-Type: application/zip\r\n\r\n').encode(), file.read_bytes(), ('\r\n--' + boundary + '--\r\n').encode()])
        return self.request('', b''.join(pieces), 'multipart/form-data; boundary=' + boundary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    p = commands.add_parser('pack'); p.add_argument('directory'); p.add_argument('--output', required=True)
    for command in ('preflight', 'publish', 'confirm', 'status'):
        p = commands.add_parser(command)
        p.add_argument('input', help='ZIP filename or batch ID')
        p.add_argument('--base', required=True)
        if command in ('preflight', 'publish'):
            p.add_argument('--collection', required=True)
            p.add_argument('--locale', default='zh-CN')
            p.add_argument('--mode', choices=['upsert', 'create-only', 'replace-version'], default='upsert')
    args = parser.parse_args()
    try:
        if args.command == 'pack':
            result = pack(args.directory, args.output)
        else:
            client = Client(args.base)
            if args.command in ('preflight', 'publish'):
                result = client.upload(args.input, args.collection, args.locale, args.mode)
                print(json.dumps(result, ensure_ascii=False), flush=True)
                if result['summary']['blocking']:
                    return 2
                if args.command == 'preflight':
                    return 0
                batch_id = result['batch']['id']
            else:
                batch_id = args.input
            path = '/' + urllib.parse.quote(batch_id, safe='')
            result = client.request(path if args.command == 'status' else path + '/confirm', None if args.command == 'status' else b'{}')
            if args.command != 'status' and result['batch']['status'] != 'completed':
                print(json.dumps(result, ensure_ascii=False)); return 2
        print(json.dumps(result, ensure_ascii=False))
        return 0
    except (ValueError, OSError, KeyError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
