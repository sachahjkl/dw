namespace Dw.Cli.Tests;

public sealed class SlugTests
{
    [Theory]
    [InlineData("descriptif cours", "descriptif-cours")]
    [InlineData("heures PSFs côté pré-réservation", "heures-psfs-cote-pre-reservation")]
    [InlineData("  Trop   d'espaces !!! ", "trop-d-espaces")]
    public void Normalize_creates_ascii_dash_slug(string input, string expected)
    {
        Assert.Equal(expected, Slug.Normalize(input));
    }
}
