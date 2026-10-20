//! Comprehensive tests and C# differential golden verification for emboss light fitting/relighting.

use against_rome_editor::emboss::*;
use against_rome_editor::terrain::{HeightOperation, SampleChange, TerrainSession};

const GOLDEN_BYTES: &[u8] = &[
0,0,0,67,193,245,159,63,63,10,224,191,125,9,180,160,148,255,239,63,128,128,126,109,111,110,102,95,88,80,72,71,
91,126,128,128,128,128,115,102,86,78,71,64,56,48,41,34,35,75,125,128,129,124,112,104,96,89,82,74,66,59,52,44,
36,29,76,127,136,123,122,114,107,100,92,84,77,70,62,54,47,40,38,96,126,140,132,125,118,110,102,95,88,80,72,65,
58,50,42,74,128,150,143,136,128,120,113,106,98,90,83,76,68,60,53,76,138,161,154,146,138,131,124,116,108,101,94,86,
78,71,64,86,149,172,164,156,149,142,134,126,119,112,104,96,89,82,74,96,160,182,174,167,160,152,144,137,130,122,114,107,
100,92,84,107,170,192,185,178,170,162,155,148,140,132,125,118,110,102,95,118,180,203,196,188,180,173,166,158,150,143,136,128,
120,113,106,128,182,214,206,198,191,184,176,168,161,154,146,138,131,124,116,130,160,218,216,209,202,194,186,179,172,164,156,149,
142,134,133,120,129,180,227,220,212,204,197,190,182,174,167,160,152,144,132,127,128,131,181,221,222,215,208,200,192,185,178,170,
154,141,128,128,128,128,130,165,185,184,176,168,161,154,146,145,147,130,128,128,128,128,126,109,111,110,102,95,88,80,72,71,
91,126,128,128,128,128,115,102,86,78,71,64,56,48,41,34,35,75,125,128,129,124,112,104,96,89,82,74,66,59,52,44,
36,29,76,127,136,123,122,114,107,100,92,84,77,70,62,54,47,40,38,96,126,140,132,125,118,110,102,93,86,80,72,65,
58,50,42,74,128,150,143,136,128,120,107,91,81,82,83,76,68,60,53,76,138,161,154,146,138,134,119,94,74,74,87,86,
78,71,64,86,149,172,164,156,150,151,145,122,95,83,91,95,89,82,74,96,160,182,174,167,161,165,173,161,134,111,105,106,
100,92,84,107,170,192,185,178,170,169,182,182,162,137,122,118,110,102,95,118,180,203,196,188,180,173,174,175,165,149,136,128,
120,113,106,128,182,214,206,198,191,184,176,170,163,154,146,138,131,124,116,130,160,218,216,209,202,194,186,179,172,164,156,149,
142,134,133,120,129,180,227,220,212,204,197,190,182,174,167,160,152,144,132,127,128,131,181,221,222,215,208,200,192,185,178,170,
154,141,128,128,128,128,130,165,185,184,176,168,161,154,146,145,147,130,128,128,128,128,138,128,113,110,102,95,88,80,72,71,
91,126,128,128,128,129,146,116,93,78,71,64,56,48,41,34,35,75,125,128,114,98,107,125,98,89,82,74,66,59,52,44,
36,29,76,127,113,104,86,113,107,100,92,84,77,70,62,54,47,40,38,96,122,129,128,125,118,110,102,93,86,80,72,65,
58,50,42,74,128,150,143,136,128,120,107,91,81,82,83,76,68,60,53,76,138,161,154,146,138,134,119,94,74,74,87,86,
78,71,64,86,149,172,164,156,150,151,145,122,95,83,91,95,89,82,74,96,160,182,174,167,161,165,173,161,134,111,105,106,
100,92,84,107,170,192,185,178,170,169,182,182,162,137,122,118,110,102,95,118,180,203,196,188,180,173,174,175,165,149,136,128,
120,113,106,128,182,214,206,198,191,184,176,170,163,154,146,138,131,124,116,130,160,218,216,209,202,194,186,179,172,164,156,149,
142,134,133,120,129,180,227,220,212,204,197,190,182,174,167,160,152,144,132,127,128,131,181,221,222,215,208,200,192,185,178,170,
154,141,128,128,128,128,130,165,185,184,176,168,161,154,146,145,147,130,128,128,128,128,138,128,113,110,102,95,88,80,72,71,
91,126,128,128,128,129,146,116,93,78,71,64,56,48,41,34,35,75,125,128,114,98,107,125,98,89,82,74,66,59,52,44,
36,29,76,127,113,104,86,113,107,100,92,84,77,70,62,54,47,40,38,96,122,129,128,125,118,110,102,93,86,80,72,65,
58,50,42,74,128,150,143,136,128,120,107,91,81,82,83,76,68,60,53,76,138,161,154,146,138,134,119,94,74,74,87,86,
78,71,64,86,149,172,164,156,150,151,145,122,95,83,91,95,89,82,74,96,160,182,174,167,161,165,173,161,134,111,105,106,
100,92,84,107,170,192,185,178,170,169,182,182,162,137,122,118,110,102,95,118,180,203,196,188,180,173,174,175,165,149,136,128,
120,113,106,128,182,214,206,198,191,184,176,170,163,154,146,138,131,124,116,130,160,218,216,209,202,194,186,179,172,164,156,149,
142,134,133,120,129,180,227,220,212,204,197,190,182,174,167,160,152,144,132,127,128,131,181,221,222,215,208,200,192,185,178,170,
154,141,128,128,128,128,130,165,185,184,176,168,161,154,146,145,147,130,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,121,117,121,128,128,128,128,128,128,128,128,128,128,128,128,126,111,95,96,116,128,128,128,128,128,
128,128,128,128,128,133,135,120,95,83,100,123,128,128,128,128,128,128,128,128,128,135,152,152,128,104,104,121,128,128,128,128,
128,128,128,128,128,133,156,173,161,136,121,123,128,128,128,128,128,128,128,128,128,128,140,160,161,145,130,128,128,128,128,128,
128,128,128,128,128,128,128,135,139,135,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,
128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128,138,128,114,110,102,95,87,80,72,71,
91,126,128,128,128,128,146,116,93,78,71,63,56,48,41,33,35,75,125,128,114,98,107,125,99,89,81,74,66,59,51,44,
36,29,76,127,113,104,86,113,107,99,92,84,77,69,62,54,47,40,38,96,123,129,129,125,117,110,102,93,86,80,72,65,
58,50,43,74,128,150,143,135,128,120,107,91,81,82,83,76,68,61,53,76,138,161,153,146,138,134,119,94,74,74,87,86,
79,71,64,86,149,171,164,156,150,151,145,122,95,83,91,95,89,82,74,97,159,182,174,167,161,165,173,161,134,111,105,106,
100,92,85,107,170,192,185,177,170,169,182,182,162,137,122,118,110,103,95,118,180,203,195,188,180,173,174,175,165,149,136,128,
121,113,106,128,182,213,206,198,191,184,176,170,163,154,146,139,131,124,116,130,160,218,216,209,202,194,187,179,172,164,157,149,
142,134,133,120,129,180,227,220,212,205,197,190,182,175,167,160,152,145,132,127,128,131,181,221,223,215,208,200,193,185,178,170,
154,141,129,128,128,128,130,165,185,184,176,169,161,154,146,145,147,130,128,128,128,128,138,128,113,110,102,95,88,80,72,71,
91,126,128,128,128,129,146,116,93,78,71,64,56,48,41,34,35,75,125,128,114,98,107,125,98,89,82,74,66,59,52,44,
36,29,76,127,113,104,86,113,107,100,92,84,77,70,62,54,47,40,38,96,122,129,128,125,118,110,102,93,86,80,72,65,
58,50,42,74,128,150,143,136,128,120,107,91,81,82,83,76,68,60,53,76,138,161,154,146,138,134,119,94,74,74,87,86,
78,71,64,86,149,172,164,156,150,151,145,122,95,83,91,95,89,82,74,96,160,182,174,167,161,165,173,161,134,111,105,106,
100,92,84,107,170,192,185,178,170,169,182,182,162,137,122,118,110,102,95,118,180,203,196,188,180,173,174,175,165,149,136,128,
120,113,106,128,182,214,206,198,191,184,176,170,163,154,146,138,131,124,116,130,160,218,216,209,202,194,186,179,172,164,156,149,
142,134,133,120,129,180,227,220,212,204,197,190,182,174,167,160,152,144,132,127,128,131,181,221,222,215,208,200,192,185,178,170,
154,141,128,128,128,128,130,165,185,184,176,168,161,154,146,145,147,130,128,128,0,0,127,67,0,0,0,0,0,0,0,0,
0,0,0,0,0,0,0,0,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,140,
140,140,140,140,140,140,140,140,0,0,127,67,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,158,188,128,
158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,
128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,
188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,
158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,
128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,
188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,
158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,
128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,158,188,128,];


#[test]
fn test_csharp_golden_differential_scenarios() {
    let mut offset = 0;
    let read_f32 = |offset: &mut usize| {
        let val = f32::from_le_bytes(GOLDEN_BYTES[*offset..*offset + 4].try_into().unwrap());
        *offset += 4;
        val
    };
    let read_f64 = |offset: &mut usize| {
        let val = f64::from_le_bytes(GOLDEN_BYTES[*offset..*offset + 8].try_into().unwrap());
        *offset += 8;
        val
    };
    let read_slice = |offset: &mut usize, len: usize| {
        let slice = &GOLDEN_BYTES[*offset..*offset + len];
        *offset += len;
        slice
    };

    // --- Scenario 1: Curved mound on 16x16 grid ---
    {
        let size = 16;
        let mut heights = vec![0u8; size * size];
        let mut emboss = vec![0u8; size * size];

        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 7.5;
                let r2 = dx * dx + dy * dy;
                let h = (120.0 - r2 * 1.5).clamp(10.0, 200.0);
                heights[y * size + x] = h.round_ties_even() as u8;
            }
        }
        for y in 0..size {
            for x in 0..size {
                let (gx, gy) = gradient(&heights, size, x, y);
                let e = (128.0 + 1.25 * gx - 1.75 * gy).clamp(0.0, 255.0);
                emboss[y * size + x] = e.round_ties_even() as u8;
            }
        }

        let expected_intercept = read_f32(&mut offset);
        let expected_slope_x = read_f32(&mut offset);
        let expected_slope_y = read_f32(&mut offset);
        let expected_r2 = read_f64(&mut offset);

        let report = fit_emboss_light(size, &heights, &emboss).unwrap();
        assert!(report.is_usable);
        assert!((report.model.intercept - expected_intercept).abs() < 1e-4);
        assert!((report.model.slope_x - expected_slope_x).abs() < 1e-4);
        assert!((report.model.slope_y - expected_slope_y).abs() < 1e-4);
        assert!((report.model.r_squared - expected_r2).abs() < 1e-5);

        let mut terrain = TerrainSession::new(size, heights.clone(), size - 1, None).unwrap();
        let mut session = EmbossSession::new(size, heights, Some(emboss.clone())).unwrap();

        assert!(!session.is_dirty(terrain.heights()));
        assert!(!session.heights_dirty(terrain.heights()));
        assert!(!session.is_relight_pending());

        // Snapshot 0: baseline (clean)
        let snap0 = read_slice(&mut offset, size * size);
        let built0 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built0, snap0);
        assert_eq!(built0, emboss);

        // Step 1: paint height (raise center)
        terrain
            .paint_height((7.5, 7.5), 3.0, HeightOperation::Raise, 20, None, 0)
            .unwrap();
        terrain.commit_stroke();
        assert!(session.is_dirty(terrain.heights()));
        assert!(session.heights_dirty(terrain.heights()));

        let snap1 = read_slice(&mut offset, size * size);
        let built1 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built1, snap1);

        // Step 2: paint height (lower corner to test clamping)
        terrain
            .paint_height((1.0, 1.0), 2.5, HeightOperation::Lower, 64, None, 0)
            .unwrap();
        terrain.commit_stroke();

        let snap2 = read_slice(&mut offset, size * size);
        let built2 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built2, snap2);

        // Step 3: commit baseline with relit emboss
        session
            .commit_baseline(terrain.heights(), Some(&built2))
            .unwrap();
        assert!(!session.is_dirty(terrain.heights()));
        assert!(!session.heights_dirty(terrain.heights()));

        let snap3 = read_slice(&mut offset, size * size);
        let built3 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built3, snap3);
        assert_eq!(built3, built2);

        // Step 4: Apply blank terrain (height 45)
        let blank_changes: Vec<SampleChange> = (0..size * size)
            .map(|index| SampleChange {
                index,
                before: terrain.heights()[index],
                after: 45,
            })
            .collect();
        terrain.apply_height_changes(&blank_changes);
        terrain.commit_stroke();
        session.apply_blank_terrain();
        assert!(session.is_relight_pending());
        assert!(session.is_dirty(terrain.heights()));

        let snap4 = read_slice(&mut offset, size * size);
        let built4 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built4, snap4);

        // Step 5: Paint on blank terrain
        terrain
            .paint_height((4.0, 4.0), 3.0, HeightOperation::Raise, 25, None, 0)
            .unwrap();
        terrain.commit_stroke();

        let snap5 = read_slice(&mut offset, size * size);
        let built5 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built5, snap5);

        // Step 6: Undo paint stroke on blank terrain
        terrain.undo();
        let snap6 = read_slice(&mut offset, size * size);
        let built6 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built6, snap6);
        assert!(session.is_relight_pending());

        // Step 7: Undo blank terrain stroke (undoes height back to step 3, pending stays true!)
        terrain.undo();
        let snap7 = read_slice(&mut offset, size * size);
        let built7 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built7, snap7);
        assert!(session.is_relight_pending());

        // Step 8: Reset to baseline (clears pending flat relight)
        session.reset_to_baseline();
        assert!(!session.is_relight_pending());
        assert!(!session.is_dirty(terrain.heights()));

        let snap8 = read_slice(&mut offset, size * size);
        let built8 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built8, snap8);
    }

    // --- Scenario 2: Constant emboss on 16x16 grid ---
    {
        let size = 16;
        let heights: Vec<u8> = (0..size * size).map(|i| (i % 100) as u8).collect();
        let const_emboss = vec![140u8; size * size];

        let expected_intercept = read_f32(&mut offset);
        let expected_slope_x = read_f32(&mut offset);
        let expected_slope_y = read_f32(&mut offset);
        let expected_r2 = read_f64(&mut offset);

        let report = fit_emboss_light(size, &heights, &const_emboss).unwrap();
        assert!(!report.is_usable);
        assert_eq!(report.model.r_squared, expected_r2);
        assert_eq!(report.model.intercept, expected_intercept);
        assert_eq!(report.model.slope_x, expected_slope_x);
        assert_eq!(report.model.slope_y, expected_slope_y);

        let mut terrain = TerrainSession::new(size, heights.clone(), size - 1, None).unwrap();
        let mut session = EmbossSession::new(size, heights, Some(const_emboss)).unwrap();

        // Snapshot 9: baseline
        let snap9 = read_slice(&mut offset, size * size);
        let built9 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built9, snap9);

        // Step 9: Paint height with unusable model -> returns saved emboss unmodified
        terrain
            .paint_height((5.0, 5.0), 3.0, HeightOperation::Raise, 30, None, 0)
            .unwrap();
        terrain.commit_stroke();

        let snap10 = read_slice(&mut offset, size * size);
        let built10 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built10, snap10);
        assert_eq!(built10, snap9);

        // Step 10: Apply blank terrain -> flat base is mean=140
        session.apply_blank_terrain();
        assert_eq!(session.flat_emboss_base(), Some(140));

        let snap11 = read_slice(&mut offset, size * size);
        let built11 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built11, snap11);
        assert!(built11.iter().all(|&v| v == 140));
    }

    // --- Scenario 3: Extreme slopes & clamping ---
    {
        let size = 16;
        let mut heights = vec![0u8; size * size];
        let mut emboss = vec![0u8; size * size];
        for y in 0..size {
            for x in 0..size {
                let h = (if x % 2 == 0 { 10 } else { 200 }) + (if y % 2 == 0 { 10 } else { 200 });
                heights[y * size + x] = h as u8;
                emboss[y * size + x] = (128 + ((x + y) % 3) * 30) as u8;
            }
        }

        let expected_intercept = read_f32(&mut offset);
        let expected_slope_x = read_f32(&mut offset);
        let expected_slope_y = read_f32(&mut offset);
        let expected_r2 = read_f64(&mut offset);

        let report = fit_emboss_light(size, &heights, &emboss).unwrap();
        assert!((report.model.intercept - expected_intercept).abs() < 1e-4);
        assert!((report.model.slope_x - expected_slope_x).abs() < 1e-4);
        assert!((report.model.slope_y - expected_slope_y).abs() < 1e-4);
        assert!((report.model.r_squared - expected_r2).abs() < 1e-5);

        let mut terrain = TerrainSession::new(size, heights.clone(), size - 1, None).unwrap();
        let session = EmbossSession::new(size, heights, Some(emboss)).unwrap();

        terrain
            .paint_height((8.0, 8.0), 6.0, HeightOperation::Raise, 64, None, 0)
            .unwrap();
        terrain.commit_stroke();

        let snap12 = read_slice(&mut offset, size * size);
        let built12 = session.build_emboss(terrain.heights()).unwrap().unwrap();
        assert_eq!(built12, snap12);
        let min_val = *built12.iter().min().unwrap();
        let max_val = *built12.iter().max().unwrap();
        assert!(min_val <= max_val);
    }

    assert_eq!(offset, GOLDEN_BYTES.len());
}

#[test]
fn test_baseline_lifecycle_and_dirty_tracking() {
    let size = 8;
    let heights = vec![50u8; size * size];
    let emboss = vec![128u8; size * size];

    let mut session = EmbossSession::new(size, heights.clone(), Some(emboss.clone())).unwrap();
    assert_eq!(session.size(), 8);
    assert!(session.has_emboss());
    assert_eq!(session.baseline_heights(), heights.as_slice());
    assert_eq!(session.saved_emboss(), Some(emboss.as_slice()));
    assert!(!session.is_dirty(&heights));
    assert!(!session.heights_dirty(&heights));
    assert!(!session.is_relight_pending());

    let mut modified_heights = heights.clone();
    modified_heights[10] = 75;
    assert!(session.heights_dirty(&modified_heights));
    assert!(session.is_dirty(&modified_heights));

    // Commit baseline without new emboss
    session.commit_baseline(&modified_heights, None).unwrap();
    assert!(!session.heights_dirty(&modified_heights));
    assert!(!session.is_dirty(&modified_heights));
    assert_eq!(session.baseline_heights(), modified_heights.as_slice());
    assert_eq!(session.saved_emboss(), Some(emboss.as_slice()));

    // Apply blank terrain and reset to baseline
    session.apply_blank_terrain();
    assert!(session.is_relight_pending());
    assert!(session.is_dirty(&modified_heights));
    session.reset_to_baseline();
    assert!(!session.is_relight_pending());
    assert!(!session.is_dirty(&modified_heights));
}

#[test]
fn test_flat_and_constant_emboss_fallback_hierarchy() {
    // 1. Usable model
    let usable_model = EmbossLightModel {
        intercept: 135.4,
        slope_x: 1.0,
        slope_y: -1.0,
        r_squared: 0.8,
    };
    let base = compute_flat_emboss_base(&usable_model, Some(&[10, 20, 30]));
    assert_eq!(base, 135);

    // 2. Unusable model with saved emboss
    let unusable_model = EmbossLightModel::none();
    let sample_emboss = vec![100, 150, 200];
    let base = compute_flat_emboss_base(&unusable_model, Some(&sample_emboss));
    assert_eq!(base, 150);

    // 3. Unusable model with None
    let base = compute_flat_emboss_base(&unusable_model, None);
    assert_eq!(base, 255);

    // 4. Missing emboss session
    let size = 8;
    let heights = vec![50u8; size * size];
    let mut session = EmbossSession::new(size, heights.clone(), None).unwrap();
    assert!(!session.has_emboss());
    assert_eq!(session.light_fit_quality(), 0.0);
    assert_eq!(session.build_emboss(&heights).unwrap(), None);

    session.apply_blank_terrain();
    assert_eq!(session.flat_emboss_base(), Some(255));
    assert_eq!(session.build_emboss(&heights).unwrap(), None);
}

#[test]
fn test_bankers_rounding_and_quantization() {
    assert_eq!(quantize_f32(126.5), 126);
    assert_eq!(quantize_f32(127.5), 128);
    assert_eq!(quantize_f32(128.5), 128);
    assert_eq!(quantize_f32(129.5), 130);

    assert_eq!(quantize_f32(-10.0), 0);
    assert_eq!(quantize_f32(300.0), 255);

    assert_eq!(quantize_f64(126.5), 126);
    assert_eq!(quantize_f64(127.5), 128);
    assert_eq!(quantize_f64(-50.0), 0);
    assert_eq!(quantize_f64(500.0), 255);
}

#[test]
fn test_preview_is_pure_and_does_not_mutate() {
    let size = 8;
    let mut heights = vec![0u8; size * size];
    let mut emboss = vec![0u8; size * size];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - 3.5;
            let dy = y as f32 - 3.5;
            let r2 = dx * dx + dy * dy;
            let h = (100.0 - r2 * 2.0).clamp(10.0, 200.0);
            heights[y * size + x] = h.round_ties_even() as u8;
        }
    }
    for y in 0..size {
        for x in 0..size {
            let (gx, gy) = gradient(&heights, size, x, y);
            let e = (128.0 + 1.5 * gx - 1.5 * gy).clamp(0.0, 255.0);
            emboss[y * size + x] = e.round_ties_even() as u8;
        }
    }
    let session = EmbossSession::new(size, heights.clone(), Some(emboss.clone())).unwrap();
    assert!(session.light().is_usable());

    let heights_clone = heights.clone();
    let preview1 = session.preview_emboss(&heights).unwrap();
    assert_eq!(heights, heights_clone);
    assert_eq!(session.saved_emboss().unwrap(), emboss.as_slice());

    let mut current_heights = heights.clone();
    current_heights[size * 3 + 3] = 180;
    let preview2 = session.preview_emboss(&current_heights).unwrap();
    assert_eq!(session.saved_emboss().unwrap(), emboss.as_slice());
    assert_ne!(preview1, preview2);
}

#[test]
fn test_invalid_dimensions_and_non_finite_parameters() {
    // Size < 2
    assert_eq!(
        fit_emboss_light(1, &[0], &[0]),
        Err(EmbossError::InvalidDimensions)
    );
    assert_eq!(
        EmbossSession::new(1, vec![0], None).err(),
        Some(EmbossError::InvalidDimensions)
    );

    // Mismatched lengths
    assert_eq!(
        fit_emboss_light(4, &[0; 16], &[0; 15]),
        Err(EmbossError::InvalidDimensions)
    );
    assert_eq!(
        EmbossSession::new(4, vec![0; 15], None).err(),
        Some(EmbossError::InvalidDimensions)
    );

    // Non-finite parameters in relight
    let invalid_light = EmbossLightModel {
        intercept: f32::NAN,
        slope_x: 0.0,
        slope_y: 0.0,
        r_squared: 0.5,
    };
    assert_eq!(
        preview_relight_delta(4, &[0; 16], &[0; 16], &[0; 16], &invalid_light),
        Err(EmbossError::InvalidParameters)
    );
    assert_eq!(
        preview_relight_flat(4, 128, &[0; 16], &invalid_light),
        Err(EmbossError::InvalidParameters)
    );

    // Coordinates out of bounds
    assert_eq!(
        calculate_gradient(&[0; 16], 4, 4, 0),
        Err(EmbossError::InvalidParameters)
    );
    assert_eq!(
        calculate_gradient(&[0; 16], 4, 0, 4),
        Err(EmbossError::InvalidParameters)
    );
}
