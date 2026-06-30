using Dw.Cli.Cli;

namespace Dw.Cli.Commands;

internal static class GuideCommand
{
    public static int Run(CommandContext context)
    {
        context.Out.WriteLine($"dw - Dev Workflow {AppVersion.InformationalVersion()}");
        context.Out.WriteLine();
        context.Out.WriteLine("Guide rapide");
        context.Out.WriteLine();
        context.Out.WriteLine("1. Initialiser un root");
        context.Out.WriteLine($"   dw init {OptionNames.Profile} ogf {OptionNames.Root} C:\\Dev\\dw {OptionNames.DryRun}");
        context.Out.WriteLine($"   dw init {OptionNames.Profile} ogf {OptionNames.Root} C:\\Dev\\dw");
        context.Out.WriteLine();
        context.Out.WriteLine("2. Verifier l'environnement");
        context.Out.WriteLine("   dw doctor");
        context.Out.WriteLine();
        context.Out.WriteLine("3. Installer l'autocompletion");
        context.Out.WriteLine("   dw completion show");
        context.Out.WriteLine("   # autres shells: bash, zsh, fish, nushell");
        context.Out.WriteLine("   . $PROFILE");
        context.Out.WriteLine();
        context.Out.WriteLine("4. Se connecter a Azure DevOps");
        context.Out.WriteLine("   dw auth login");
        context.Out.WriteLine("   dw auth status");
        context.Out.WriteLine();
        context.Out.WriteLine("5. Lire un work item sans rien modifier");
        context.Out.WriteLine($"   dw ado work-item 55201 {OptionNames.Project} ha");
        context.Out.WriteLine($"   dw ado context 55201 {OptionNames.Project} ha");
        context.Out.WriteLine();
        context.Out.WriteLine("6. Demarrer un workspace de travail");
        context.Out.WriteLine($"   dw task start <workItemId> {OptionNames.Project} ha {OptionNames.Slug} \"titre court\" {OptionNames.Only} front,back");
        context.Out.WriteLine();
        context.Out.WriteLine("7. Donner le contexte a l'agent");
        context.Out.WriteLine("   dw agent context");
        context.Out.WriteLine();
        context.Out.WriteLine("8. Inspecter la fin en dry-run");
        context.Out.WriteLine($"   dw task finish {OptionNames.Workspace} <workspace>");
        context.Out.WriteLine();
        context.Out.WriteLine("9. Terminer vraiment quand tout est pret");
        context.Out.WriteLine($"   dw task finish {OptionNames.Workspace} <workspace> {OptionNames.Execute} {OptionNames.CreatePr}");
        context.Out.WriteLine();
        context.Out.WriteLine("Notes");
        context.Out.WriteLine($"- Sans {OptionNames.Execute}, task finish est toujours un dry-run.");
        context.Out.WriteLine($"- Pour init, {OptionNames.NoSave} cree les fichiers mais ne modifie pas les settings; {OptionNames.DryRun} ne cree rien.");
        context.Out.WriteLine("- Un task workspace cible un seul projet dw: ha ou he.");
        context.Out.WriteLine("- Les secrets SQL se stockent via dw secret set <key>.");
        context.Out.WriteLine("- Les schemas JSON sont generes dans <root>\\schemas et references par config\\*.json.");
        return 0;
    }
}
