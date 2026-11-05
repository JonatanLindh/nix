# Pull one named blob out of a MediaTek Windows driver's mtkwlan.dat.
#
# The container's index holds, per entry: a NUL-terminated name, NUL
# padding, an optional 14-digit build timestamp, then (4-byte aligned)
# little-endian u32 offset and size of the payload.
#
# usage: mtkwlan-extract.py <mtkwlan.dat> <name> <out>
import struct
import sys

dat, name, out = sys.argv[1], sys.argv[2].encode(), sys.argv[3]
data = open(dat, "rb").read()

if data.count(name + b"\0") != 1:
    sys.exit(f"expected exactly one index entry for {name!r}")
pos = data.index(name + b"\0") + len(name)

while data[pos] == 0:
    pos += 1
stamp = data[pos : pos + 14]
if len(stamp) == 14 and stamp.isdigit():
    print(f"{name.decode()}: build {stamp.decode()}", file=sys.stderr)
    pos += 14
pos = (pos + 3) & ~3

offset, size = struct.unpack_from("<II", data, pos)
if size == 0 or offset + size > len(data):
    sys.exit(f"bad payload bounds: offset={offset} size={size}")

with open(out, "wb") as f:
    f.write(data[offset : offset + size])
