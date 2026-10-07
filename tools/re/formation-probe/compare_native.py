"""Compare C# --layout JSON predictions to previously ALR-matched game-units.png anchors.
No fitting of spacing/origin/angle; only unknown member identity is assigned optimally.
"""
import json, math, sys
from pathlib import Path

observed = [(384,277),(448,245),(480,325),(512,277),(512,181),
            (545,229),(576,181),(608,261),(640,213),(672,261)]
data = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
layout = next(item for item in data['layouts'] if item['count'] == 10)
predicted = [(512 + .5*(p['x']-p['z']), 245 + .25*(p['x']+p['z'])) for p in layout['offsets']]

def match(measurements):
    # Inject observations into distinct predictions; nine-point mode leaves one prediction unused.
    dp = {0: (0, [])}
    for mask in range(1 << len(predicted)):
        if mask not in dp: continue
        cost, pairs = dp[mask]; i = mask.bit_count()
        if i == len(measurements): continue
        for j, point in enumerate(predicted):
            if mask >> j & 1: continue
            candidate = cost + sum((point[k]-measurements[i][k])**2 for k in (0,1))
            next_mask = mask | 1 << j
            if next_mask not in dp or candidate < dp[next_mask][0]:
                dp[next_mask] = (candidate, pairs + [j])
    cost, pairs = min(v for mask, v in dp.items() if mask.bit_count() == len(measurements))
    errors = [math.dist(predicted[j], p) for j, p in zip(pairs, measurements)]
    return {'rms': math.sqrt(cost/len(measurements)), 'mean': sum(errors)/len(errors),
            'max': max(errors), 'predictionIndices': pairs, 'errors': errors}

print(json.dumps({'sourceSha256': data['sha256'], 'segmentCount': data['segmentCount'],
                  'predictedPixels': predicted, 'tenIncludingOccluded': match(observed),
                  'nineClear': match(observed[:4]+observed[5:])}, indent=2))
