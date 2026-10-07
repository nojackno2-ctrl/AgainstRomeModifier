"""Reject a specific four-column rectangle hypothesis against measured anchors.
This is NOT a native formDef=0 prediction. Standard library only.
"""
import math
observed=[(384,277),(448,245),(480,325),(512,277),(512,181),(545,229),(576,181),(608,261),(640,213),(672,261)]
predicted=[]
for i in range(10):
    local_x=-192+128*(i%4); local_z=-128+128*(i//4)
    world_x=-local_z; world_z=-local_x
    predicted.append((512+(world_x-world_z)/2,245+(world_x+world_z)/4))
dp={0:(0,[])}
for mask in range(1<<10):
    if mask not in dp: continue
    cost,pairs=dp[mask]; i=mask.bit_count()
    if i==10: continue
    for j in range(10):
        if mask>>j&1: continue
        next_mask=mask|1<<j
        candidate=cost+sum((predicted[i][k]-observed[j][k])**2 for k in (0,1))
        if next_mask not in dp or candidate<dp[next_mask][0]: dp[next_mask]=(candidate,pairs+[j])
print('four-column rectangle',predicted)
print('minimum bijective matching RMS pixels',math.sqrt(dp[1023][0]/10))
print('matching observation indices',dp[1023][1])
