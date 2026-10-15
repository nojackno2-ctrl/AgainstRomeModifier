//! cl_epara.ini patcher and clean baseline ported from
//! src.Core/Core/Patches/EparaPatcher.cs and src.Core/Core/Services/CleanEparaBaseline.cs.

use crate::lzss::compress_pfil;
use crate::patch_text::TextDocument;
use std::io::{self, Error, ErrorKind};

pub const VARIANCE_SECTION: &str = "[ProjectileVarianceOnMove]";
pub const ACCURACY_VARIANCE_VALUE: &str = "0.0";
pub const INIT_SPEED_SECTION: &str = "[ProjectileInitSpeedFactor]";
pub const INIT_SPEED_REACH_MULTIPLIER: f64 = 1.08;

pub struct EparaPatcher;

impl EparaPatcher {
    pub fn get_patched_bytes(original: &[u8], ranged_range_3x: bool) -> io::Result<Vec<u8>> {
        if !ranged_range_3x {
            return Ok(original.to_vec());
        }

        let doc = TextDocument::read(original)?;
        let mut lines = doc.lines.clone();

        Self::patch_section_value(&mut lines, VARIANCE_SECTION, |_| ACCURACY_VARIANCE_VALUE.to_string())?;
        Self::patch_section_value(&mut lines, INIT_SPEED_SECTION, |val| {
            let res = val * INIT_SPEED_REACH_MULTIPLIER;
            // Format to match C# "0.###"
            let s = format!("{:.3}", res);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        })?;

        doc.write_with_lines(&lines)
    }

    fn patch_section_value(
        lines: &mut [String],
        section: &str,
        transform: impl Fn(f64) -> String,
    ) -> io::Result<()> {
        for i in 0..lines.len() {
            if !lines[i].trim().eq_ignore_ascii_case(section) {
                continue;
            }

            for j in (i + 1)..lines.len() {
                let trimmed = lines[j].trim();
                if trimmed.is_empty() || trimmed.starts_with(';') {
                    continue;
                }
                if trimmed.starts_with('[') {
                    break;
                }

                let val = trimmed.parse::<f64>().map_err(|_| {
                    Error::new(
                        ErrorKind::InvalidData,
                        format!("cl_epara.ini {} value line cannot be parsed: '{}'", section, trimmed),
                    )
                })?;

                let new_val_str = transform(val);
                lines[j] = lines[j].replace(trimmed, &new_val_str);
                return Ok(());
            }
        }

        Err(Error::new(
            ErrorKind::InvalidData,
            format!("cl_epara.ini missing value line for section {}", section),
        ))
    }
}

pub struct CleanEparaBaseline;

impl CleanEparaBaseline {
    pub const TEXT: &'static str = ";Multiplikator fuer FormationsRotationTempo\r
;1.0 entspricht maximalem RotationsTempo wenn alle Figuren die FormationsPosition halten\r
;\r
;1.0 bis 500.0\r
[FormationRotationFaktor]\r
500.0\r
\r
;Multiplikator fuer FormationsBewegungsTempo derjenigen Muckel\r
;die die Formation gerade einhalten, so koennen nicht einhaltende Muckel wieder aufholen\r
;\r
;0.01 bis 1.00\r
[FormationSpeedFaktor]\r
0.7\r
\r
;maximale Anzahl an Pfadfindungsversuchen pro Muckel wenn Ziel bei Stillstand der\r
;Formation durch Kollision belegt ist\r
;\r
;2..16\r
[FormationPathDepth]\r
2\r
\r
;Zeit in ms die eine Figur in einer Formationsbewegung wartet, wenn sie auf eine Kollision trifft\r
;Defautl=1000\r
;0..X\r
[FormationCollisionWaitTime]\r
150\r
\r
;Gibt in % an, wieviel eine Heohendifferenz von einem Pattern zum naechsten die\r
;Globale Hoehenrichtungsbeleuchtung beeinflusst\r
;\r
;0..100\r
[GlobalFloorLightIntensity]\r
10\r
\r
;Gibt die Intensitдt an von 0 bis 100% an, mit welcher der 3-dimensionale Bewegungsvektor\r
;genutzt wird, es ergibt sich fьr die Bewegungsgeschwindigkeit eine Konvexkombination (baryzentrisch)\r
;speed= 3Dspeed*Intensity + 2Dspeed*(100%-Intensity)  (default: intensity=100)\r
;\r
[MoveVector3DIntensity]\r
100\r
\r
;Gibt die Geschwindigkeit der Bewegung der Wolkenspiegelungstextur an\r
;0=keine 1=langsam 16=normal 256=schnell 4095=maximal (Default=16)\r
;\r
[CloudReflectMoveSpeed]\r
16\r
\r
;Gibt den Angriffswertfaktor an, mit dem der normale Angriffswert im aktiven Zustand 'Berserker'\r
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des AW, 0.5 bewirkt eine Halbierung\r
[BerserkerAWfaktor]\r
2.0\r
\r
;Gibt den Damagewertfaktor an (Nahkampf), mit dem der normale Schaden im aktiven Zustand 'Berserker'\r
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Schadens, 0.5 bewirkt eine Halbierung\r
[BerserkerDAMfaktor]\r
2.0\r
\r
;Gibt den Verteigungswertfaktor an (Nahkampf), mit dem der normale Verteigungswert im aktiven Zustand 'Berserker'\r
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des VW, 0.5 bewirkt eine Halbierung, 0.0 bewirkt eine Setzung zu VW=0\r
[BerserkerVWfaktor]\r
0.0\r
\r
;Gibt den Schussradiusfaktor an (Fernkampfwaffe 1+2), mit dem der normale Schussradius im aktiven Zustand 'Schuetzengeschick'\r
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Radius, 0.5 bewirkt eine Halbierung\r
[SchuetzengeschickRADfaktor]\r
1.2\r
\r
;Gibt den Schadensfaktor an (saemtlicher Schaeden), mit dem der normale Schaden im aktiven Zustand 'Schutzschild'\r
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Schadens, 0.5 bewirkt eine Halbierung\r
[SchutzschildDAMfaktor]\r
0.8\r
\r
;Gibt den Schadensfaktor an (Waffe 0), mit dem der normale Schaden im aktiven Zustand 'Donnerschlag'\r
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Schadens, 0.5 bewirkt eine Halbierung\r
[DonnerschlagDAMfaktor]\r
1.5\r
\r
;Gibt den Geschwindigkeitabschussfaktor fьr Geschosse an (Waffe 1-7) ausgehend vom ursprьnglich eingestellten Faktor 1.0\r
;annдhernde Korrektur der Flugbahnlдnge durch Multiplikation mit 1.52 des zugehцrigen Parameter Ysub in den ParticleDefaults\r
[ProjectileInitSpeedFactor]\r
1.5\r
\r
;gibt die Unsicherheit der Vorhalte bei Projektilattacken an (nur fuer sich bewegende Ziele)\r
;0.0 bedeutet: keine Unsicherheit, das Projektil trifft mit Vorhalte absolut prдzise\r
;0.5 bedeutet: eine Abweichung von bis zu 0.5*3*MoveSpeed_des_Ziels (in Pattern) ist moeglich\r
;1.0 bedeutet: eine Abweichung von bis zu 1.0*3*MoveSpeed_des_Ziels (in Pattern) ist moeglich\r
;1.5 bedeutet: eine Abweichung von bis zu 1.5*3*MoveSpeed_des_Ziels (in Pattern) ist moeglich\r
;Default =0.5\r
[ProjectileVarianceOnMove]\r
0.5\r
\r
;gibt den Winkel zwischen Zielposition und prognostizierter Zielposition in Grad an, ab dem die Vorhalte abgeschaltet wird\r
;Vermeidung zu starker Abweichung zwischen Projektilflugrichtung und Blickrichtung des feuernden Objektes\r
;Default=45\r
[ProjectileVarianceMaximumAngle]\r
45\r
\r
;Gibt den Bereich an, in dem die Distanz zwischen Zielposition und prognostizierter Zielposition variieren darf, bevor die\r
;Vorhalte abgeschaltet wird\r
;0.4 bedeutet: Distanz zur Vorhalteposition muss zwischen der (1-0.4)=0.6 und (1+0.4)=1.4'fachen Distanz zur Zielposition liegen\r
;Default=0.4\r
[ProjectileVarianceDistanceRange]\r
0.4\r
\r
;Gibt die Zeit in ms, die als maximale Zeitdifferenz zwischen zwei logischen Frames an\r
;Default=3000\r
;(Wer hier rumfummelt und nicht genau weiss was er tut, bekommt die Figer abgehackt :-)\r
[MaxLogicFrameTime]\r
3000";

    pub fn create_bytes() -> io::Result<Vec<u8>> {
        let mut header = [0u8; 64];
        header[..4].copy_from_slice(b"PFIL");
        compress_pfil(Self::TEXT.as_bytes(), Some(&header))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epara_range3x_zeros_variance_and_scales_speed() {
        let text = ";comment\r\n[ProjectileInitSpeedFactor]\r\n1.5\r\n\r\n;lead scatter\r\n[ProjectileVarianceOnMove]\r\n0.5\r\n\r\n[ProjectileVarianceMaximumAngle]\r\n45\r\n";
        let patched = EparaPatcher::get_patched_bytes(text.as_bytes(), true).unwrap();
        let s = String::from_utf8(patched).unwrap();
        assert!(s.contains("[ProjectileVarianceOnMove]\r\n0.0\r\n"));
        assert!(s.contains("[ProjectileInitSpeedFactor]\r\n1.62\r\n"));
        assert!(s.contains("[ProjectileVarianceMaximumAngle]\r\n45\r\n"));

        let untouched = EparaPatcher::get_patched_bytes(text.as_bytes(), false).unwrap();
        assert_eq!(untouched, text.as_bytes());
    }

    #[test]
    fn clean_baseline_creates_valid_pfil() {
        let bytes = CleanEparaBaseline::create_bytes().unwrap();
        assert!(crate::lzss::is_pfil(&bytes));
        let decomp = crate::lzss::decompress_pfil(&bytes).unwrap();
        assert_eq!(decomp, CleanEparaBaseline::TEXT.as_bytes());
    }
}
