#!/usr/bin/env python3
"""Read-only native objects-to-animation slot consistency diagnostics."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct

from inventory_game_directory import pfil_prefix
from probe_map_pools import inspect
from probe_pua import require

COLUMN_WIDTHS = (2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 4, 2, 2, 4)


def decode(data):
    if data.startswith(b'PFIL@'):
        require(len(data) >= 64, 'short PFIL header')
        size = struct.unpack_from('<i', data, 16)[0]
        require(0 <= size <= 4 * 1024 * 1024, 'decoded pool exceeds analysis limit')
        return pfil_prefix(data, limit=size)[1]
    return data


def inspect_links(objects, anim):
    objects, anim = decode(objects), decode(anim)
    require(len(objects) >= 16, 'short objects header')
    version, count, name0, name1 = struct.unpack_from('<4I', objects)
    require(version == 1 and count <= 14000 and name0 == name1 == 30,
            'unsupported objects layout')
    require(len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'objects length mismatch')
    animation = inspect(anim, 'anim')
    anim_count = animation['slots']
    active_anim = {i for i in range(anim_count) if anim[8 + i * 21] != 0}
    used = Counter()
    invalid, inactive = [], []
    active_objects = unlinked = 0
    for slot in range(count):
        if objects[16 + slot * 79] == 0:
            continue
        active_objects += 1
        # Native CHECK consumes this u16 as the signed high half of a packed word.
        target = struct.unpack_from('<h', objects, 16 + slot * 79 + 71)[0]
        if target == -1:
            unlinked += 1
        elif not 0 <= target < anim_count:
            invalid.append({'object_slot': slot, 'anim_slot': target})
        else:
            used[target] += 1
            if target not in active_anim:
                inactive.append({'object_slot': slot, 'anim_slot': target})
    return {'objects_decoded_sha256': hashlib.sha256(objects).hexdigest(),
            'anim_decoded_sha256': hashlib.sha256(anim).hexdigest(),
            'object_slots': count, 'anim_slots': anim_count,
            'active_objects': active_objects, 'active_anim_slots': len(active_anim),
            'unlinked_objects': unlinked, 'linked_objects': sum(used.values()),
            'out_of_range_links': invalid, 'inactive_target_links': inactive,
            'shared_anim_slots': {str(k): n for k, n in sorted(used.items()) if n > 1},
            'active_anim_without_active_object_link': sorted(active_anim - used.keys())}


def scan(root):
    require((root / 'MAPS').is_dir(), 'missing MAPS directory')
    reports, errors, missing = [], [], []
    for directory in sorted((root / 'MAPS').iterdir()):
        if not directory.is_dir() or directory.is_symlink():
            continue
        paths = [directory / 'DATA' / name for name in ('objects.dat', 'anim.dat')]
        if not all(p.is_file() for p in paths):
            missing.append(directory.name)
            continue
        try:
            require(all(p.resolve().is_relative_to(root.resolve()) for p in paths), 'pool path escapes source')
            reports.append({'map': directory.name, **inspect_links(*(p.read_bytes() for p in paths))})
        except (OSError, ValueError, struct.error) as exc:
            errors.append({'map': directory.name, 'error': str(exc)})
    return {'maps': reports, 'errors': errors, 'missing_pool_pairs': missing,
            'limitations': ['one objects record field and anim validity byte only; other pools and UID relations not checked',
                            'shared slots and unreferenced active slots are observations, not proven corruption',
                            'stricter version/capacity/length checks than native; PFIL trailing integrity not checked',
                            'no game execution, original file writes or runtime success inference']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists() or output.is_relative_to(args.root.resolve()):
        parser.error('output must be new and outside source directory')
    try:
        result = scan(args.root)
        with output.open('x', encoding='utf-8') as stream:
            json.dump(result, stream, indent=2)
            stream.write('\n')
        print(json.dumps({'maps': len(result['maps']), 'errors': len(result['errors']),
                          'out_of_range_links': sum(len(m['out_of_range_links']) for m in result['maps']),
                          'inactive_target_links': sum(len(m['inactive_target_links']) for m in result['maps'])}))
        return 1 if result['errors'] else 0
    except (OSError, ValueError, struct.error) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
