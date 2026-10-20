using System.Text.Json;
using AgainstRomeModifier;

// bci-oracle <request.json> <response.json>
// Request: {Sites:[{DataHex,Pattern:[int?]}], Pfil:[{PlainHex,TemplateHex}], Decode:[hex]}
var req = JsonDocument.Parse(File.ReadAllText(args[0])).RootElement;
var sites = new List<int[]>();
foreach (var c in req.GetProperty("Sites").EnumerateArray())
{
    var data = Convert.FromHexString(c.GetProperty("DataHex").GetString()!);
    var pattern = c.GetProperty("Pattern").EnumerateArray()
        .Select(e => e.ValueKind == JsonValueKind.Null ? (int?)null : e.GetInt32()).ToArray();
    sites.Add(BciPattern.FindAllBciWordPatternSites(data, pattern).ToArray());
}
var compressed = new List<string>();
foreach (var c in req.GetProperty("Pfil").EnumerateArray())
{
    var plain = Convert.FromHexString(c.GetProperty("PlainHex").GetString()!);
    var template = Convert.FromHexString(c.GetProperty("TemplateHex").GetString()!);
    compressed.Add(Convert.ToHexString(GameLZSS.CompressPfil(plain, template)));
}
var decoded = new List<string>();
foreach (var h in req.GetProperty("Decode").EnumerateArray())
    decoded.Add(Convert.ToHexString(GameLZSS.DecompressPfil(Convert.FromHexString(h.GetString()!))));
File.WriteAllText(args[1], JsonSerializer.Serialize(new { Sites = sites, Compressed = compressed, Decoded = decoded }));
return 0;
