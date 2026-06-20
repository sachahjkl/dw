using System.Text;

namespace Dw.Cli.Platform;

internal interface IFileSystem
{
    bool DirectoryExists(string path);
    bool FileExists(string path);
    void CreateDirectory(string path);
    string ReadAllText(string path);
    void WriteAllText(string path, string content);
    IEnumerable<string> EnumerateFiles(string path, string searchPattern, SearchOption searchOption);
}

internal sealed class RealFileSystem : IFileSystem
{
    public bool DirectoryExists(string path) => Directory.Exists(path);

    public bool FileExists(string path) => File.Exists(path);

    public void CreateDirectory(string path) => Directory.CreateDirectory(path);

    public string ReadAllText(string path) => File.ReadAllText(path, Encoding.UTF8);

    public void WriteAllText(string path, string content)
    {
        var directory = Path.GetDirectoryName(path);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }

        File.WriteAllText(path, content, new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
    }

    public IEnumerable<string> EnumerateFiles(string path, string searchPattern, SearchOption searchOption)
        => Directory.Exists(path)
            ? Directory.EnumerateFiles(path, searchPattern, searchOption)
            : [];
}
