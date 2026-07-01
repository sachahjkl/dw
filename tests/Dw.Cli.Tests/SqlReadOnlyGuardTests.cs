namespace Dw.Cli.Tests;

public sealed class SqlReadOnlyGuardTests
{
    [Theory]
    [InlineData("select top 10 * from dbo.Users")]
    [InlineData("-- comment\r\nselect 1")]
    [InlineData("with cte as (select 1 as Id) select * from cte")]
    public void Validate_allows_readonly_queries(string sql)
    {
        Assert.True(SqlReadOnlyGuard.Validate(sql).IsAllowed);
    }

    [Theory]
    [InlineData("delete from dbo.Users")]
    [InlineData("select * from dbo.Users; drop table dbo.Users")]
    [InlineData("exec dbo.DoSomething")]
    [InlineData("update dbo.Users set Name = 'x'")]
    public void Validate_blocks_dangerous_queries(string sql)
    {
        var result = SqlReadOnlyGuard.Validate(sql);

        Assert.False(result.IsAllowed);
        Assert.NotNull(result.Reason);
    }
}
