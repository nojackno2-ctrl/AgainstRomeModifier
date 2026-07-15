using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Core.Services;

internal static class CleanEparaBaseline
{
    internal const string Text = @";Multiplikator fuer FormationsRotationTempo
;1.0 entspricht maximalem RotationsTempo wenn alle Figuren die FormationsPosition halten
;
;1.0 bis 500.0
[FormationRotationFaktor]
500.0

;Multiplikator fuer FormationsBewegungsTempo derjenigen Muckel
;die die Formation gerade einhalten, so koennen nicht einhaltende Muckel wieder aufholen
;
;0.01 bis 1.00
[FormationSpeedFaktor]
0.7

;maximale Anzahl an Pfadfindungsversuchen pro Muckel wenn Ziel bei Stillstand der
;Formation durch Kollision belegt ist
;
;2..16
[FormationPathDepth]
2

;Zeit in ms die eine Figur in einer Formationsbewegung wartet, wenn sie auf eine Kollision trifft
;Defautl=1000
;0..X
[FormationCollisionWaitTime]
150

;Gibt in % an, wieviel eine Heohendifferenz von einem Pattern zum naechsten die
;Globale Hoehenrichtungsbeleuchtung beeinflusst
;
;0..100
[GlobalFloorLightIntensity]
10

;Gibt die Intensitдt an von 0 bis 100% an, mit welcher der 3-dimensionale Bewegungsvektor
;genutzt wird, es ergibt sich fьr die Bewegungsgeschwindigkeit eine Konvexkombination (baryzentrisch)
;speed= 3Dspeed*Intensity + 2Dspeed*(100%-Intensity)  (default: intensity=100)
;
[MoveVector3DIntensity]
100

;Gibt die Geschwindigkeit der Bewegung der Wolkenspiegelungstextur an
;0=keine 1=langsam 16=normal 256=schnell 4095=maximal (Default=16)
;
[CloudReflectMoveSpeed]
16

;Gibt den Angriffswertfaktor an, mit dem der normale Angriffswert im aktiven Zustand 'Berserker'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des AW, 0.5 bewirkt eine Halbierung
[BerserkerAWfaktor]
2.0

;Gibt den Damagewertfaktor an (Nahkampf), mit dem der normale Schaden im aktiven Zustand 'Berserker'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Schadens, 0.5 bewirkt eine Halbierung
[BerserkerDAMfaktor]
2.0

;Gibt den Verteigungswertfaktor an (Nahkampf), mit dem der normale Verteigungswert im aktiven Zustand 'Berserker'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des VW, 0.5 bewirkt eine Halbierung, 0.0 bewirkt eine Setzung zu VW=0
[BerserkerVWfaktor]
0.0

;Gibt den Schussradiusfaktor an (Fernkampfwaffe 1+2), mit dem der normale Schussradius im aktiven Zustand 'Schuetzengeschick'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Radius, 0.5 bewirkt eine Halbierung
[SchuetzengeschickRADfaktor]
1.2

;Gibt den Schadensfaktor an (saemtlicher Schaeden), mit dem der normale Schaden im aktiven Zustand 'Schutzschild'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Schadens, 0.5 bewirkt eine Halbierung
[SchutzschildDAMfaktor]
0.8

;Gibt den Schadensfaktor an (Waffe 0), mit dem der normale Schaden im aktiven Zustand 'Donnerschlag'
;multipliziert wird, z.B. bewirkt 2.0 eine Verdopplung des Schadens, 0.5 bewirkt eine Halbierung
[DonnerschlagDAMfaktor]
1.5

;Gibt den Geschwindigkeitabschussfaktor fьr Geschosse an (Waffe 1-7) ausgehend vom ursprьnglich eingestellten Faktor 1.0
;annдhernde Korrektur der Flugbahnlдnge durch Multiplikation mit 1.52 des zugehцrigen Parameter Ysub in den ParticleDefaults
[ProjectileInitSpeedFactor]
1.5

;gibt die Unsicherheit der Vorhalte bei Projektilattacken an (nur fuer sich bewegende Ziele)
;0.0 bedeutet: keine Unsicherheit, das Projektil trifft mit Vorhalte absolut prдzise
;0.5 bedeutet: eine Abweichung von bis zu 0.5*3*MoveSpeed_des_Ziels (in Pattern) ist moeglich
;1.0 bedeutet: eine Abweichung von bis zu 1.0*3*MoveSpeed_des_Ziels (in Pattern) ist moeglich
;1.5 bedeutet: eine Abweichung von bis zu 1.5*3*MoveSpeed_des_Ziels (in Pattern) ist moeglich
;Default =0.5
[ProjectileVarianceOnMove]
0.5

;gibt den Winkel zwischen Zielposition und prognostizierter Zielposition in Grad an, ab dem die Vorhalte abgeschaltet wird
;Vermeidung zu starker Abweichung zwischen Projektilflugrichtung und Blickrichtung des feuernden Objektes
;Default=45
[ProjectileVarianceMaximumAngle]
45

;Gibt den Bereich an, in dem die Distanz zwischen Zielposition und prognostizierter Zielposition variieren darf, bevor die
;Vorhalte abgeschaltet wird
;0.4 bedeutet: Distanz zur Vorhalteposition muss zwischen der (1-0.4)=0.6 und (1+0.4)=1.4'fachen Distanz zur Zielposition liegen
;Default=0.4
[ProjectileVarianceDistanceRange]
0.4

;Gibt die Zeit in ms, die als maximale Zeitdifferenz zwischen zwei logischen Frames an
;Default=3000
;(Wer hier rumfummelt und nicht genau weiss was er tut, bekommt die Figer abgehackt :-)
[MaxLogicFrameTime]
3000";

    internal static byte[] CreateBytes()
    {
        byte[] header = new byte[64];
        header[0] = (byte)'P';
        header[1] = (byte)'F';
        header[2] = (byte)'I';
        header[3] = (byte)'L';
        return GameLZSS.CompressPfil(PatchText.GameEncoding.GetBytes(Text), header);
    }
}
