namespace Dw.Cli.Commands;

internal static class SecretCommand
{
    public static int Run(CommandContext context, string[] args)
    {
        var sub = args.FirstOrDefault()?.ToLowerInvariant();
        return sub switch
        {
            "set" => Set(context, args.Skip(1).ToArray()),
            "get" => Get(context, args.Skip(1).ToArray()),
            "delete" => Delete(context, args.Skip(1).ToArray()),
            _ => Help(context)
        };
    }

    private static int Set(CommandContext context, string[] args)
    {
        var key = args.FirstOrDefault() ?? throw new DwException("Usage: dw secret set <key>", 2);
        var secret = CommandOptions.OptionValue(args, "--value");
        var fromEnv = CommandOptions.OptionValue(args, "--from-env");
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

    private static int Get(CommandContext context, string[] args)
    {
        var key = args.FirstOrDefault() ?? throw new DwException("Usage: dw secret get <key>", 2);
        var value = SecretStoreFactory.Create().Get(key);
        context.Out.WriteLine(value is null ? "Secret introuvable." : "Secret present.");
        return value is null ? 1 : 0;
    }

    private static int Delete(CommandContext context, string[] args)
    {
        var key = args.FirstOrDefault() ?? throw new DwException("Usage: dw secret delete <key>", 2);
        SecretStoreFactory.Create().Delete(key);
        context.Out.WriteLine("Secret supprime si present.");
        return 0;
    }

    private static int Help(CommandContext context)
    {
        CliCatalog.WriteCommandHelp(context.Out, "secret");
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
