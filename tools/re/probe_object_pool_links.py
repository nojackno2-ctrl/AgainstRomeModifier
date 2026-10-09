#!/usr/bin/env python3
"""Read-only action and hirarchy slot diagnostics for native map objects."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct

from probe_map_pools import inspect
from probe_object_anim_links import COLUMN_WIDTHS, decode
from probe_pua import require


def inspect_links(objects, action, hirarchy):
    objects, action, hirarchy = map(decode, (objects, action, hirarchy))
    require(len(objects) >= 16, 'short objects header')
    version, count, n0, n1 = struct.unpack_from('<4I', objects)
    require(version == 1 and count <= 14000 and n0 == n1 == 30, 'unsupported objects layout')
    require(len(objects) == 16 + count * (79 + sum(COLUMN_WIDTHS)), 'objects length mismatch')
    action_count = inspect(action, 'action')['slots']
    require(len(hirarchy) >= 12, 'short hirarchy header')
    version, groups, width = struct.unpack_from('<3I', hirarchy)
    require(version == 1 and groups <= 3200 and width == 50, 'unsupported hirarchy layout')
    require(len(hirarchy) == 12 + groups * 105, 'hirarchy length mismatch')
    active_objects = {i for i in range(count) if objects[16 + i * 79] != 0}
    active_action = {i for i in range(action_count) if action[12 + i * 25] != 0}
    active_groups = {i for i in range(groups) if hirarchy[12 + i * 103] != 0}
    # Reader maps objects column 4 to runtime +0x28, the hirarchy backlink.
    group_column = 16 + count * 79 + count * sum(COLUMN_WIDTHS[:4])
    def group_link(slot):
        return struct.unpack_from('<h', objects, group_column + slot * 2)[0]
    action_used = Counter()
    issues, members, group_links = [], {}, {}
    for slot in sorted(active_objects):
        target = struct.unpack_from('<h', objects, 16 + slot * 79 + 77)[0]
        if target != -1:
            if not 0 <= target < action_count:
                issues.append({'kind': 'action-out-of-range', 'object': slot, 'target': target})
            else:
                action_used[target] += 1
                if target not in active_action:
                    issues.append({'kind': 'action-inactive', 'object': slot, 'target': target})
        target = group_link(slot)
        if target != -1:
            group_links[slot] = target
            if not 0 <= target < groups:
                issues.append({'kind': 'hirarchy-out-of-range', 'object': slot, 'target': target})
            elif target not in active_groups:
                issues.append({'kind': 'hirarchy-inactive', 'object': slot, 'target': target})
    for group in sorted(active_groups):
        offset = 12 + group * 103
        size = struct.unpack_from('<h', hirarchy, offset + 1)[0]
        if not 0 <= size <= 50:
            issues.append({'kind': 'hirarchy-member-count', 'group': group, 'count': size})
            continue
        values = struct.unpack_from('<' + 'h' * size, hirarchy, offset + 3)
        members[group] = values
        for slot in values:
            if not 0 <= slot < count:
                issues.append({'kind': 'member-out-of-range', 'group': group, 'object': slot})
            elif slot not in active_objects:
                issues.append({'kind': 'member-inactive', 'group': group, 'object': slot})
            elif group_link(slot) != group:
                issues.append({'kind': 'member-backlink-mismatch', 'group': group, 'object': slot,
                               'backlink': group_link(slot)})
        if len(set(values)) != len(values):
            issues.append({'kind': 'duplicate-members', 'group': group})
    for slot, group in group_links.items():
        if group in members and slot not in members[group]:
            issues.append({'kind': 'backlink-member-mismatch', 'group': group, 'object': slot})
    return {'decoded_sha256': {k: hashlib.sha256(v).hexdigest() for k, v in
                              [('objects', objects), ('action', action), ('hirarchy', hirarchy)]},
            'active_objects': len(active_objects), 'active_action': len(active_action),
            'active_hirarchy': len(active_groups), 'action_linked_objects': sum(action_used.values()),
            'hirarchy_linked_objects': len(group_links),
            'shared_action_slots': {str(k): n for k, n in sorted(action_used.items()) if n > 1},
            'active_action_without_object_link': sorted(active_action - action_used.keys()),
            'hirarchy_member_count_histogram': dict(sorted(Counter(map(len, members.values())).items())),
            'issues': issues}


def scan(root):
    require((root / 'MAPS').is_dir(), 'missing MAPS directory')
    reports, errors, missing = [], [], []
    for directory in sorted((root / 'MAPS').iterdir()):
        if not directory.is_dir() or directory.is_symlink():
            continue
        paths = [directory / 'DATA' / name for name in ('objects.dat', 'action.dat', 'hirarchy.dat')]
        if not all(p.is_file() for p in paths):
            missing.append(directory.name)
            continue
        try:
            require(all(p.resolve().is_relative_to(root.resolve()) for p in paths), 'pool path escapes source')
            reports.append({'map': directory.name, **inspect_links(*(p.read_bytes() for p in paths))})
        except (OSError, ValueError, struct.error) as exc:
            errors.append({'map': directory.name, 'error': str(exc)})
    return {'maps': reports, 'errors': errors, 'missing_pools': missing,
            'limitations': ['partial link diagnostics, not proof of corruption or runtime loading success',
                            'no UID checks, other pools, tail-word semantics or action-word semantics',
                            'strict writer version/capacity/length; PFIL compressed trailing integrity not checked',
                            'no game execution, extraction or original file writes']}


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
        kinds = Counter(i['kind'] for m in result['maps'] for i in m['issues'])
        print(json.dumps({'maps': len(result['maps']), 'errors': len(result['errors']), 'issues': kinds}))
        return 1 if result['errors'] else 0
    except (OSError, ValueError, struct.error) as exc:
        print(str(exc))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
