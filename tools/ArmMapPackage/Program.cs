using AgainstRomeMapEditor.Modules.Packaging;

bool forceExport = args.Length == 4 && args[0] == "export" && args[3] == "--force";
if ((!forceExport && args.Length != 3) || args[0] is not ("install" or "export"))
{
    Console.Error.WriteLine("Usage: ArmMapPackage install <bundle.zip> <game-root> | export <map-directory> <bundle.zip> [--force]");
    return 2;
}
try
{
    if (args[0] == "install")
    {
        var result = ModBundleExporter.InstallFromZip(args[1], args[2]);
        Console.WriteLine($"Installed {result.Manifest.Title}: ENDL_{result.InstalledSlot:000} ({result.ExtractedFileCount} payload files)");
        Console.WriteLine(result.DirectoryPath);
        Console.WriteLine($"Playability remains unverified: preflight {result.PreflightReport?.ErrorCount} errors, {result.PreflightReport?.WarningCount} warnings.");
    }
    else
    {
        var result = ModBundleExporter.ExportToZip(args[1], args[2], new() { ForceExportOnErrors = forceExport });
        Console.WriteLine($"Exported {result.Manifest.Title}: {result.FileCount} payload files");
        Console.WriteLine(result.OutputZipPath);
        Console.WriteLine($"Preflight: {result.PreflightReport.ErrorCount} errors, {result.PreflightReport.WarningCount} warnings.");
    }
    return 0;
}
catch (Exception ex) when (ex is IOException or InvalidOperationException or ArgumentException or NotSupportedException or System.Text.Json.JsonException)
{
    Console.Error.WriteLine(ex.Message);
    return 1;
}
