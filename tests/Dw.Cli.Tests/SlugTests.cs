namespace Dw.Cli.Tests;

public sealed class SlugTests
{
    [Theory]
    [InlineData("descriptif cours", "descriptif-cours")]
    [InlineData("heures PSFs côté pré-réservation", "heures-psfs-cote-pre-reservation")]
    [InlineData("  Trop   d'espaces !!! ", "trop-d-espaces")]
    [InlineData("ceci est un Test hehe", "ceci-est-un-test-hehe")]
    public void Normalize_creates_ascii_dash_slug(string input, string expected)
    {
        Assert.Equal(expected, Slug.Normalize(input));
    }

    [Fact]
    public void FromPhraseOrFallback_uses_fallback_when_phrase_becomes_empty()
    {
        Assert.Equal("work-item-55222", Slug.FromPhraseOrFallback("!!!", "work item 55222"));
    }
}
