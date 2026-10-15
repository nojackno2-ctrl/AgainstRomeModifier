using AgainstRomeMapEditor;

// Frozen operation recipe, executed against the actual C# implementation.
// Output is concatenated complete layer snapshots, not self-generated expectations.
using var output = new BinaryWriter(File.Create(args[0]));
foreach (int size in new[] {193,257})
{
    byte[] initial = Enumerable.Range(0,size*size).Select(i => (byte)((i*17+i/size*13)%256)).ToArray();
    foreach (TerrainFalloffType falloff in Enum.GetValues<TerrainFalloffType>())
    foreach (bool caps in new[] {false,true})
    {
        byte[] h = initial.ToArray();
        var changes = TerrainRampTool.CalculateRamp(h,size,2.5f,4.25f,size-3.75f,size-5.5f,new TerrainRampParameters {
            Width=5.5f,Falloff=3.75f,StartHeight=12.5f,EndHeight=241.5f,FalloffType=falloff,RoundCaps=caps });
        foreach(var c in changes) h[c.Index]=c.After;
        output.Write(h);
    }
    int cs=size-1;
    byte[] collision=Enumerable.Range(0,cs*cs).Select(i=>(byte)(i%11==0?90:0)).ToArray();
    var session = new TerrainHeightEditSession(size,initial,null,cs,collision);
    void Snapshot() {output.Write(session.Heights.ToArray());output.Write(session.Collision!.ToArray());}
    foreach(TerrainHeightOperation op in Enum.GetValues<TerrainHeightOperation>())
    {
        session.PaintHeight(19.5f,21.25f,8.75f,op,17,73,-12345);
        session.PaintHeight(22.0f,20.0f,7.25f,op,9,81,-12345);
        session.CommitStroke(); Snapshot();
    }
    session.PaintWater(20.5f,20.75f,9.25f,64,12,110);session.CommitStroke();Snapshot();
    session.Undo();Snapshot();session.Undo();Snapshot();session.Redo();Snapshot();
    session.PaintHeight(20,20,5,TerrainHeightOperation.Lower,7);session.CancelStroke();Snapshot();
    session.CommitBaseline(null);Snapshot();
}
for(int y=-8;y<=8;y++) for(int x=-8;x<=8;x++) output.Write(TerrainHeightEditSession.RoughnessNoise(x,y,-12345));
