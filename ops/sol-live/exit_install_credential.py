"""Stage a validated operator-owned input in a private installer directory."""
import os
import stat
import sys


def copy_input(path, output, uid):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        st = os.fstat(fd)
        if (not stat.S_ISREG(st.st_mode) or st.st_uid != uid or st.st_nlink != 1
                or stat.S_IMODE(st.st_mode) != 0o600 or not 0 < st.st_size <= 16384):
            raise ValueError("Expected a single operator-owned regular file with mode 0600.")
        with os.fdopen(fd, "rb", closefd=False) as stream:
            contents = stream.read(16385)
        if len(contents) != st.st_size:
            raise ValueError("Credential length changed during installation.")
        output.write(contents)
        output.flush()
        os.fsync(output.fileno())
    finally:
        os.close(fd)


if __name__ == "__main__":
    source, destination, uid = sys.argv[1:]
    os.umask(0o077)
    with open(destination, "xb") as output:
        copy_input(source, output, int(uid))
