#!/usr/bin/env python3
"""Reference implementation of adiungere-track-fp/1 and adiungere-annexb/1.

Usage:  track-fingerprint.py RECORDING TRACK_INDEX [--annexb]

Prints the fingerprint of one track as the tool prints it, from nothing but the
standard library, so that a stranger can check a manifest without any code from
this project. The rules are stated in docs/integrity.md; this file is the shortest
faithful rendering of them and is compared with the product on every pull request.
"""

import hashlib
import struct
import sys


def boxes(data, start, end):
    """Yields (type, payload_start, payload_end) for the boxes in data[start:end]."""
    at = start
    while at + 8 <= end:
        size, kind = struct.unpack(">I4s", data[at : at + 8])
        header = 8
        if size == 1:
            (size,) = struct.unpack(">Q", data[at + 8 : at + 16])
            header = 16
        elif size == 0:
            size = end - at
        yield kind, at + header, at + size
        at += size


def child(data, start, end, path):
    """Follows a path of box types, first match at each level."""
    for kind in path:
        for found, payload_start, payload_end in boxes(data, start, end):
            if found == kind:
                start, end = payload_start, payload_end
                break
        else:
            raise SystemExit(f"no {kind.decode()} box")
    return start, end


def table(data, start, end, kind, fmt):
    """Reads a full box holding a count then fixed-size entries."""
    payload_start, payload_end = child(data, start, end, [kind])
    (count,) = struct.unpack(">I", data[payload_start + 4 : payload_start + 8])
    width = struct.calcsize(fmt)
    at = payload_start + 8
    return [struct.unpack(fmt, data[at + i * width : at + (i + 1) * width]) for i in range(count)]


def samples(data, stbl_start, stbl_end):
    """Every sample's (offset, size) in decode order."""
    runs = table(data, stbl_start, stbl_end, b"stsc", ">III")
    try:
        offsets = [o for (o,) in table(data, stbl_start, stbl_end, b"co64", ">Q")]
    except SystemExit:
        offsets = [o for (o,) in table(data, stbl_start, stbl_end, b"stco", ">I")]
    stsz_start, _ = child(data, stbl_start, stbl_end, [b"stsz"])
    constant, count = struct.unpack(">II", data[stsz_start + 4 : stsz_start + 12])
    sizes = [constant] * count if constant else [
        s for (s,) in struct.iter_unpack(">I", data[stsz_start + 12 : stsz_start + 12 + 4 * count])
    ]
    located, index = [], 0
    for position, (first, per_chunk, _) in enumerate(runs):
        last = runs[position + 1][0] - 1 if position + 1 < len(runs) else len(offsets)
        for chunk in range(first, last + 1):
            offset = offsets[chunk - 1]
            for _ in range(per_chunk):
                located.append((offset, sizes[index]))
                offset += sizes[index]
                index += 1
    return located


def main():
    if len(sys.argv) not in (3, 4):
        raise SystemExit(__doc__)
    path, track_index = sys.argv[1], int(sys.argv[2])
    annexb = len(sys.argv) == 4 and sys.argv[3] == "--annexb"
    data = open(path, "rb").read()

    moov = child(data, 0, len(data), [b"moov"])
    traks = [b for b in boxes(data, *moov) if b[0] == b"trak"]
    _, trak_start, trak_end = traks[track_index]
    stbl = child(data, trak_start, trak_end, [b"mdia", b"minf", b"stbl"])
    stsd_start, stsd_end = child(data, *stbl, [b"stsd"])
    _, entry_start, entry_end = next(boxes(data, stsd_start + 8, stsd_end))
    entry_kind = data[entry_start - 4 : entry_start]
    fixed = 78 if entry_kind in (b"avc1", b"avc3", b"hvc1", b"hev1") else 28
    config = next(
        (b for b in boxes(data, entry_start + fixed, entry_end) if b[0] in (b"avcC", b"esds")),
        None,
    )
    if config is None:
        raise SystemExit("no decoder configuration")
    configuration = data[config[1] : config[2]]

    payload = hashlib.sha256()
    for offset, size in samples(data, *stbl):
        payload.update(data[offset : offset + size])
    print(f"adiungere-track-fp/1 payload {payload.hexdigest()}")
    print(f"adiungere-track-fp/1 configuration {hashlib.sha256(configuration).hexdigest()}")

    if not annexb:
        return
    if config[0] != b"avcC":
        raise SystemExit("the elementary stream digest is defined for avcC tracks only")
    length_size = (configuration[4] & 3) + 1
    at, sps, pps = 6, [], []
    for _ in range(configuration[5] & 0x1F):
        (n,) = struct.unpack(">H", configuration[at : at + 2])
        sps.append(configuration[at + 2 : at + 2 + n])
        at += 2 + n
    for _ in range(configuration[at]):
        (n,) = struct.unpack(">H", configuration[at + 1 : at + 3])
        pps.append(configuration[at + 3 : at + 3 + n])
        at += 3 + n

    stream, new_refresh = hashlib.sha256(), True
    for offset, size in samples(data, *stbl):
        sample, at, written, sps_seen, pps_seen = data[offset : offset + size], 0, 0, False, False
        while at < len(sample):
            n = int.from_bytes(sample[at : at + length_size], "big")
            unit = sample[at + length_size : at + length_size + n]
            at += length_size + n
            kind = unit[0] & 0x1F
            if kind == 7:
                sps_seen = new_refresh = True
            elif kind == 8:
                pps_seen = new_refresh = True
            if not new_refresh and kind == 5 and unit[1] & 0x80:
                new_refresh = True
            if new_refresh and kind == 5 and not sps_seen and not pps_seen:
                for ps in sps + pps:
                    stream.update(b"\0\0\0\1" + ps)
                    written += 1
                new_refresh = False
            elif new_refresh and kind == 5 and sps_seen and not pps_seen:
                for ps in pps:
                    stream.update(b"\0\0\0\1" + ps)
                    written += 1
            stream.update((b"\0\0\0\1" if written == 0 else b"\0\0\1") + unit)
            written += 1
    print(f"adiungere-annexb/1 stream {stream.hexdigest()}")


if __name__ == "__main__":
    main()
