namespace Dw.Cli.Commands;

internal static class SecretCommand
{
    internal static int Set(CommandContext context, string key, string? secret, string? fromEnv)
    {
        if (!string.IsNullOrWhiteSpace(fromEnv))
        {
            secret = Environment.GetEnvironmentVariable(fromEnv)
                     ?? throw new DwException($"Variable d'environnement introuvable: {fromEnv}");
        }

        if (secret is null)
        {
            context.Out.Write("Secret: ");
            secret = ReadSecretFromConsole();
        }

        SecretStoreFactory.Create().Set(key, secret);
        context.Out.WriteLine("Secret enregistre dans Windows Credential Manager.");
        return 0;
    }

    internal static int Get(CommandContext context, string key)
    {
        var value = SecretStoreFactory.Create().Get(key);
        context.Out.WriteLine(value is null ? "Secret introuvable." : "Secret present.");
        return value is null ? 1 : 0;
    }

    internal static int Delete(CommandContext context, string key)
    {
        SecretStoreFactory.Create().Delete(key);
        context.Out.WriteLine("Secret supprime si present.");
        return 0;
    }

    private static string ReadSecretFromConsole()
    {
        var chars = new List<char>();
        while (true)
        {
            var key = Console.ReadKey(intercept: true);
            if (key.Key == ConsoleKey.Enter)
            {
                Console.WriteLine();
                return new string(chars.ToArray());
            }

            if (key.Key == ConsoleKey.Backspace)
            {
                if (chars.Count > 0)
                {
                    chars.RemoveAt(chars.Count - 1);
                }
                continue;
            }

            chars.Add(key.KeyChar);
        }
    }

}
