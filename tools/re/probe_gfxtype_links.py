#!/usr/bin/env python3
"""Read-only objects record+73 to native gfxtype slot diagnostics."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct

from probe_map_pools import inspect
from probe_object_anim_links import COLUMN_WIDTHS, decode
from probe_pua import require


def inspect_links(objects, gfxtype):
    objects, gfxtype = decode(objects), decode(gfxtype)
    require(len(objects) >= 16, 'short objects header')
    version, count, n0, n1 = struct.unpack_from('<4I', objects)
    require(version == 1 and count <= 14000 and n0 == n1 == 30, 'unsupported objects layout')
    require(len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'objects length mismatch')
    gfx_count = inspect(gfxtype, 'gfxtype')['slots']
    active = {i for i in range(gfx_count) if gfxtype[8 + i * 15] != 0}
    used, issues = Counter(), []
    active_objects = unlinked = 0
    for slot in range(count):
        if objects[16 + slot * 79] == 0:
            continue
        active_objects += 1
        target = struct.unpack_from('<h', objects, 16 + slot * 79 + 73)[0]
        if target == -1:
            unlinked += 1
        elif not 0 <= target < gfx_count:
            issues.append({'kind': 'out-of-range', 'object': slot, 'target': target})
        else:
            used[target] += 1
            if target not in active:
                issues.append({'kind': 'inactive-target', 'object': slot, 'target': target})
    return {'objects_decoded_sha256': hashlib.sha256(objects).hexdigest(),
            'gfxtype_decoded_sha256': hashlib.sha256(gfxtype).hexdigest(),
            'active_objects': active_objects, 'active_gfxtype': len(active),
            'unlinked_objects': unlinked, 'linked_objects': sum(used.values()),
            'shared_gfxtype_slots': {str(k): n for k, n in sorted(used.items()) if n > 1},
            'active_gfxtype_without_object_link': sorted(active - used.keys()), 'issues': issues}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    if args.output.exists() or args.output.resolve().is_relative_to(args.root.resolve()):
        parser.error('output must be new and outside source directory')
    reports, errors, missing = [], [], []
    try:
        require((args.root / 'MAPS').is_dir(), 'missing MAPS directory')
        for directory in sorted((args.root / 'MAPS').iterdir()):
            if not directory.is_dir() or directory.is_symlink():
                continue
            paths = [directory / 'DATA' / n for n in ('objects.dat', 'gfxtype.dat')]
            if not all(p.is_file() for p in paths):
                missing.append(directory.name)
                continue
            try:
                require(all(p.resolve().is_relative_to(args.root.resolve()) for p in paths), 'pool path escapes source')
                reports.append({'map': directory.name, **inspect_links(*(p.read_bytes() for p in paths))})
            except (OSError, ValueError, struct.error) as exc:
                errors.append({'map': directory.name, 'error': str(exc)})
        result = {'maps': reports, 'errors': errors, 'missing_pools': missing,
                  'limitations': ['one slot link only; not UID or whole-map consistency',
                                  'no gfxtype field semantics or runtime loading proof',
                                  'strict writer layout; PFIL trailing integrity not checked',
                                  'shared or unreferenced slots are observations, not proven corruption']}
        with args.output.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, indent=2)
            stream.write('\n')
        print(json.dumps({'maps': len(reports), 'errors': len(errors),
                          'issues': Counter(i['kind'] for m in reports for i in m['issues'])}))
        return 1 if errors else 0
    except (OSError, ValueError, struct.error) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
