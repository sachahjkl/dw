using System.Reflection;

namespace Dw.Cli.Bootstrap;

internal static class SchemaResourceWriter
{
    private static readonly string[] SchemaFiles =
    [
        "projects.schema.json",
        "workflow.schema.json",
        "databases.schema.json",
        "release.schema.json"
    ];

    public static void WriteIfMissing(IFileSystem fileSystem, string root)
        => Write(fileSystem, root, overwrite: false);

    public static void Write(IFileSystem fileSystem, string root, bool overwrite)
    {
        var schemaDirectory = Path.Combine(root, "schemas");
        fileSystem.CreateDirectory(schemaDirectory);

        foreach (var fileName in SchemaFiles)
        {
            var content = ReadEmbeddedSchema(fileName);
            if (overwrite)
            {
                fileSystem.WriteAllText(Path.Combine(schemaDirectory, fileName), content);
            }
            else
            {
                InitFileWriter.WriteIfMissing(fileSystem, Path.Combine(schemaDirectory, fileName), content);
            }
        }
    }

    private static string ReadEmbeddedSchema(string fileName)
    {
        var resourceName = $"schemas/{fileName}";
        var assembly = Assembly.GetExecutingAssembly();
        using var stream = assembly.GetManifestResourceStream(resourceName)
            ?? throw new DwException($"Schema embarque introuvable: {resourceName}");
        using var reader = new StreamReader(stream);
        return reader.ReadToEnd();
    }
}

internal static class InitFileWriter
{
    public static void WriteIfMissing(IFileSystem fileSystem, string path, string content)
    {
        if (!fileSystem.FileExists(path))
        {
            fileSystem.WriteAllText(path, content);
        }
    }
}
