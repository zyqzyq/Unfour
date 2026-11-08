import json, os, stat, sys, tempfile

def patch():
    request = json.loads(sys.stdin.buffer.read().decode('utf-8'))
    path = request['path']
    limit = request['maxFileBytes']
    before = os.lstat(path)
    if not stat.S_ISREG(before.st_mode):
        raise ValueError('not a regular file')
    with open(path, 'rb') as source:
        data = source.read(limit + 1)
    if len(data) > limit:
        print('SSH_PATCH_FILE_TOO_LARGE', file=sys.stderr)
        return 4
    data.decode('utf-8')
    search = request['search'].encode('utf-8')
    replace = request['replace'].encode('utf-8')
    count = data.count(search)
    print('__UNFOUR_MATCH_COUNT__', count)
    if count == 0 or (count > 1 and not request['allowMultiple']):
        return 3
    if len(data) + count * (len(replace) - len(search)) > limit:
        print('SSH_PATCH_FILE_TOO_LARGE', file=sys.stderr)
        return 4
    updated = data.replace(search, replace)
    fd, temporary = tempfile.mkstemp(prefix='.unfour-patch-', dir=os.path.dirname(os.path.abspath(path)))
    try:
        with os.fdopen(fd, 'wb') as output:
            output.write(updated)
            output.flush()
            os.fsync(output.fileno())
        if hasattr(os, 'chown'):
            owner = os.stat(temporary)
            if (owner.st_uid, owner.st_gid) != (before.st_uid, before.st_gid):
                os.chown(temporary, before.st_uid, before.st_gid)
        os.chmod(temporary, stat.S_IMODE(before.st_mode))
        after = os.lstat(path)
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise ValueError('file changed')
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    print('__UNFOUR_PATCHED__', count)
    return 0

try:
    sys.exit(patch())
except Exception:
    print('SSH_PATCH_FAILED: target must be an unchanged, writable regular UTF-8 file', file=sys.stderr)
    sys.exit(1)
