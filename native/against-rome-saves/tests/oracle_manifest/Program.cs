using System.Globalization;
using System.IO.Compression;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using AgainstRomeModifier.Core.Services;

namespace ManifestOracle;

public sealed class OracleBatchRequest
{
    [JsonPropertyName("writes")]
    public List<WriteRequest> Writes { get; set; } = new();

    [JsonPropertyName("format_times")]
    public List<string> FormatTimes { get; set; } = new();

    [JsonPropertyName("deserializes")]
    public List<DeserializeRequest> Deserializes { get; set; } = new();

    [JsonPropertyName("zip_reads")]
    public List<ZipReadRequest> ZipReads { get; set; } = new();
}

public sealed class WriteRequest
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("folder")]
    public string Folder { get; set; } = "";

    [JsonPropertyName("title")]
    public string Title { get; set; } = "";

    [JsonPropertyName("level")]
    public string Level { get; set; } = "";

    [JsonPropertyName("backup_time")]
    public string BackupTime { get; set; } = "";
}

public sealed class DeserializeRequest
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("raw_base64")]
    public string RawBase64 { get; set; } = "";
}

public sealed class ZipReadRequest
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("zip_path")]
    public string ZipPath { get; set; } = "";
}

public sealed class OracleBatchResponse
{
    [JsonPropertyName("writes")]
    public List<WriteResponse> Writes { get; set; } = new();

    [JsonPropertyName("format_times")]
    public List<FormatTimeResponse> FormatTimes { get; set; } = new();

    [JsonPropertyName("deserializes")]
    public List<DeserializeResponse> Deserializes { get; set; } = new();

    [JsonPropertyName("zip_reads")]
    public List<ZipReadResponse> ZipReads { get; set; } = new();
}

public sealed class WriteResponse
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("json")]
    public string Json { get; set; } = "";

    [JsonPropertyName("orig_folder")]
    public string OrigFolder { get; set; } = "";

    [JsonPropertyName("title")]
    public string Title { get; set; } = "";

    [JsonPropertyName("level")]
    public string Level { get; set; } = "";

    [JsonPropertyName("backup_time")]
    public string BackupTime { get; set; } = "";
}

public sealed class FormatTimeResponse
{
    [JsonPropertyName("input")]
    public string Input { get; set; } = "";

    [JsonPropertyName("formatted")]
    public string Formatted { get; set; } = "";
}

public sealed class DeserializeResponse
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("success")]
    public bool Success { get; set; }

    [JsonPropertyName("orig_folder")]
    public string? OrigFolder { get; set; }

    [JsonPropertyName("title")]
    public string? Title { get; set; }

    [JsonPropertyName("level")]
    public string? Level { get; set; }

    [JsonPropertyName("backup_time")]
    public string? BackupTime { get; set; }

    [JsonPropertyName("error")]
    public string? Error { get; set; }
}

public sealed class ZipReadResponse
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("entry_exists")]
    public bool EntryExists { get; set; }

    [JsonPropertyName("success")]
    public bool Success { get; set; }

    [JsonPropertyName("orig_folder")]
    public string? OrigFolder { get; set; }

    [JsonPropertyName("title")]
    public string? Title { get; set; }

    [JsonPropertyName("level")]
    public string? Level { get; set; }

    [JsonPropertyName("backup_time")]
    public string? BackupTime { get; set; }

    [JsonPropertyName("formatted_time")]
    public string? FormattedTime { get; set; }

    [JsonPropertyName("error")]
    public string? Error { get; set; }
}

internal static class Program
{
    public static int Main(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: ManifestOracle <request.json> <response.json>");
            return 1;
        }

        string requestPath = args[0];
        string responsePath = args[1];

        if (!File.Exists(requestPath))
        {
            Console.Error.WriteLine($"Request file not found: {requestPath}");
            return 2;
        }

        string requestJson = File.ReadAllText(requestPath, Encoding.UTF8);
        OracleBatchRequest? request = JsonSerializer.Deserialize<OracleBatchRequest>(requestJson);
        if (request == null)
        {
            Console.Error.WriteLine("Failed to deserialize request");
            return 3;
        }

        var response = new OracleBatchResponse();

        // 1. Process Write Cases
        foreach (var w in request.Writes)
        {
            DateTime dt;
            if (!DateTime.TryParse(w.BackupTime, CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind, out dt))
            {
                dt = DateTime.Parse(w.BackupTime, CultureInfo.InvariantCulture);
            }

            string json = SaveBackupManifestCodec.Write(w.Folder, w.Title, w.Level, dt);
            SaveBackupManifest? decoded = JsonSerializer.Deserialize<SaveBackupManifest>(json);

            response.Writes.Add(new WriteResponse
            {
                Id = w.Id,
                Json = json,
                OrigFolder = decoded?.OrigFolder ?? "",
                Title = decoded?.Title ?? "",
                Level = decoded?.Level ?? "",
                BackupTime = decoded?.BackupTime ?? ""
            });
        }

        // 2. Process FormatTime Cases
        foreach (var ft in request.FormatTimes)
        {
            string formatted = SaveBackupManifestCodec.FormatTime(ft);
            response.FormatTimes.Add(new FormatTimeResponse
            {
                Input = ft,
                Formatted = formatted
            });
        }

        // 3. Process Deserialize Cases
        foreach (var d in request.Deserializes)
        {
            byte[] bytes = Convert.FromBase64String(d.RawBase64);
            try
            {
                using var ms = new MemoryStream(bytes);
                using var reader = new StreamReader(ms, Encoding.UTF8, detectEncodingFromByteOrderMarks: true);
                string text = reader.ReadToEnd();
                SaveBackupManifest? m = JsonSerializer.Deserialize<SaveBackupManifest>(text);
                response.Deserializes.Add(new DeserializeResponse
                {
                    Id = d.Id,
                    Success = true,
                    OrigFolder = m?.OrigFolder,
                    Title = m?.Title,
                    Level = m?.Level,
                    BackupTime = m?.BackupTime
                });
            }
            catch (Exception ex)
            {
                response.Deserializes.Add(new DeserializeResponse
                {
                    Id = d.Id,
                    Success = false,
                    Error = $"{ex.GetType().Name}: {ex.Message}"
                });
            }
        }

        // 4. Process ZipRead Cases
        foreach (var z in request.ZipReads)
        {
            bool entryExists = false;
            try
            {
                using ZipArchive archive = ZipFile.OpenRead(z.ZipPath);
                entryExists = archive.GetEntry(SaveBackupManifestCodec.EntryName) != null;
                SaveBackupManifest? m = SaveBackupManifestCodec.Read(archive);
                if (m != null)
                {
                    response.ZipReads.Add(new ZipReadResponse
                    {
                        Id = z.Id,
                        EntryExists = entryExists,
                        Success = true,
                        OrigFolder = m.OrigFolder ?? "",
                        Title = m.Title ?? "",
                        Level = m.Level ?? "",
                        BackupTime = m.BackupTime ?? "",
                        FormattedTime = SaveBackupManifestCodec.FormatTime(m.BackupTime ?? "")
                    });
                }
                else
                {
                    response.ZipReads.Add(new ZipReadResponse
                    {
                        Id = z.Id,
                        EntryExists = entryExists,
                        Success = true,
                        OrigFolder = null,
                        Title = null,
                        Level = null,
                        BackupTime = null,
                        FormattedTime = null
                    });
                }
            }
            catch (Exception ex)
            {
                response.ZipReads.Add(new ZipReadResponse
                {
                    Id = z.Id,
                    EntryExists = entryExists,
                    Success = false,
                    Error = $"{ex.GetType().Name}: {ex.Message}"
                });
            }
        }

        string responseJson = JsonSerializer.Serialize(response, JsonDefaults.Indented);
        File.WriteAllText(responsePath, responseJson, Encoding.UTF8);

        Console.WriteLine($"Manifest differential oracle completed: {response.Writes.Count} writes, {response.FormatTimes.Count} format_times, {response.Deserializes.Count} deserializes, {response.ZipReads.Count} zip_reads.");
        return 0;
    }
}
