use super::camera::{MapStartCamera, StartCameraPrimitive};
use super::cutscene::CutsceneSequence;
use super::spline::CameraTrackSplinePlanner;
use against_rome_scenario::{Action, BciImage, Event};

/// 已核對 EXE ABI 的底層呼叫片段；完整序列仍為實驗性、未接入存檔。
/// 不推測航點 Zoom、Pitch、Yaw 與原生引擎參數的對應。見 cinematic-camera.md。
pub struct CinematicBciCompiler;

impl CinematicBciCompiler {
    pub const IS_EXPERIMENTAL: bool = true;
    pub const IS_WIRED_TO_LEVEL_SCRIPT: bool = false;

    /// 產生位置與縮放 statement 的 CODE 片段 (Vec<i32>)，並登錄 image 常數；不附加 CODE、修改 main 或注入等待。
    /// 三個位置參數沿用原生引擎順序，zoom 為原生 0..9，不是 CameraWaypoint 的預覽距離。
    pub fn compile_camera_calls(
        image: &mut BciImage,
        engine_x: f64,
        engine_y: f64,
        engine_z: f64,
        engine_zoom: f64,
    ) -> Result<Vec<i32>, String> {
        for &val in &[engine_x, engine_y, engine_z] {
            if !val.is_finite() || val.abs() > f32::MAX as f64 {
                return Err("位置必須能以原生 float 表示。".into());
            }
        }
        if !engine_zoom.is_finite() || !(0.0..=9.0).contains(&engine_zoom) {
            return Err("原生縮放範圍為 0..9；預覽距離尚無換算證據。".into());
        }

        let mut words = Vec::new();
        fn push_double(words: &mut Vec<i32>, value: f64) {
            let bits = value.to_bits();
            words.push(67);
            words.push(bits as i32);
            words.push((bits >> 32) as i32);
        }

        // VM 0x5b1700 reads arguments from the stack top; each double occupies two words.
        push_double(&mut words, engine_z);
        push_double(&mut words, engine_y);
        push_double(&mut words, engine_x);

        let set_engine_pos_const = image.native("s_lgcSetEnginePos")?;
        words.push(128);
        words.push(set_engine_pos_const);
        words.push(73);
        words.push(-6);

        push_double(&mut words, engine_zoom);
        let set_engine_zoom_const = image.native("s_lgcSetEngineZoom")?;
        words.push(128);
        words.push(set_engine_zoom_const);
        words.push(73);
        words.push(-2);

        Ok(words)
    }

    /// 編譯原版地圖開局使用的 s_centerToPos(x, z) 呼叫。
    /// 參數順序：BCI 棧頂為 X (arg0)，次層為 Z (arg1)。
    /// 單位為世界座標 (0..16383)，由引擎內部查詢地表高度並轉化為相機空間位置。
    pub fn compile_center_to_pos_call(
        image: &mut BciImage,
        x: i32,
        z: i32,
    ) -> Result<Vec<i32>, String> {
        if !(0..=16383).contains(&x) {
            return Err("X 必須在 0..16383 範圍內。".into());
        }
        if !(0..=16383).contains(&z) {
            return Err("Z 必須在 0..16383 範圍內。".into());
        }

        let center_to_pos_const = image.native("s_centerToPos")?;
        let words = vec![
            66,
            z,
            66,
            x,
            128,
            center_to_pos_const,
            73,
            -2,
        ];
        Ok(words)
    }

    /// 依據 MapStartCamera 設定編譯開局相機 bytecode words。
    /// 支援 s_centerToPos + s_lgcSetEngineZoom 或 s_lgcSetEnginePos + s_lgcSetEngineZoom。
    pub fn compile_start_camera_calls(
        image: &mut BciImage,
        camera: &MapStartCamera,
    ) -> Result<Vec<i32>, String> {
        camera.validate()?;

        match camera.primitive {
            StartCameraPrimitive::CenterToPos => {
                let mut words = Self::compile_center_to_pos_call(
                    image,
                    camera.x.round() as i32,
                    camera.z.round() as i32,
                )?;
                let bits = camera.zoom.to_bits();
                words.push(67);
                words.push(bits as i32);
                words.push((bits >> 32) as i32);

                let set_engine_zoom_const = image.native("s_lgcSetEngineZoom")?;
                words.push(128);
                words.push(set_engine_zoom_const);
                words.push(73);
                words.push(-2);
                Ok(words)
            }
            StartCameraPrimitive::DirectEnginePos => {
                Self::compile_camera_calls(image, camera.x, camera.y, camera.z, camera.zoom)
            }
        }
    }

    /// 與原版 ENDL 的訊息 statement 相同；字串使用遊戲編碼，沒有字幕時長/語音/強制彈出語意。
    pub fn compile_message_call(image: &mut BciImage, text: &str) -> Result<Vec<i32>, String> {
        let text_index = image.game_string(text)?;
        let show_text_box_const = image.native("s_showTextBox")?;
        let words = vec![
            76,
            text_index,
            66,
            0,
            128,
            show_text_box_const,
            73,
            -2,
        ];
        Ok(words)
    }

    /// 僅把字幕轉為一次性 Message 事件。DelaySeconds 是從開局算起，非前一字幕的相對延遲。
    /// 不執行相機、輸入鎖定、黑邊、語音、部隊或後續觸發；沒有字幕就沒有事件。
    pub fn compile_to_scenario_events(sequence: &CutsceneSequence) -> Vec<Event> {
        let mut sorted_subtitles = sequence.subtitles.clone();
        sorted_subtitles.sort_by(|a, b| {
            a.start_time_seconds
                .partial_cmp(&b.start_time_seconds)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        sorted_subtitles
            .into_iter()
            .enumerate()
            .map(|(index, sub)| {
                let text = if sub.speaker.trim().is_empty() {
                    sub.text
                } else {
                    format!("[{}] {}", sub.speaker, sub.text)
                };
                let delay = sub.start_time_seconds.round() as i32;
                Event {
                    name: format!("{}_字幕_{}", sequence.name, index + 1),
                    delay_seconds: delay.max(0),
                    repeat: false,
                    enabled: true,
                    actions: vec![Action {
                        kind: 0, // Message
                        text,
                        ..Default::default()
                    }],
                    conditions: vec![],
                }
            })
            .collect()
    }

    /// 人類可讀的預覽規劃，不是 IPR 原始碼或可執行 BCI；所有輸出皆為註解。
    pub fn generate_bci_script_text(sequence: &CutsceneSequence, sample_steps: usize) -> String {
        let mut out = String::new();
        out.push_str("// EXPERIMENTAL / UNWIRED: planning preview only; not executable IPR or BCI.\n");
        out.push_str(&format!("// Sequence: {} ({})\n", sequence.name, sequence.id));
        out.push_str("// Verified ABI: s_lgcSetEnginePos v(ddd); s_lgcSetEngineZoom v(d), native zoom 0..9.\n");
        out.push_str("// Preview Zoom/Pitch/Yaw mapping, timing, letterbox, player control, restore, FX and orders are unverified.\n");

        let planner = CameraTrackSplinePlanner::new(&sequence.camera_waypoints);
        let duration = sequence.calculate_total_duration().max(planner.total_duration());
        let steps = sample_steps.clamp(2, 10000);

        for i in 0..=steps {
            let t = (i as f32) * duration / (steps as f32);
            let pose = planner.evaluate(t);
            out.push_str(&format!(
                "// t={:.2}s preview position=({:.1}, {:.1}, {:.1}) pitch={:.1} yaw={:.1} preview distance={:.1}\n",
                pose.time_seconds,
                pose.position[0],
                pose.position[1],
                pose.position[2],
                pose.pitch_degrees,
                pose.yaw_degrees,
                pose.zoom
            ));
        }

        out
    }
}
