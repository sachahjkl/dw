package paritytest_test

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/paritytest"
)

func differentialBinaries(t *testing.T) (string, string) {
	t.Helper()
	oracle := os.Getenv("DW_ORACLE_BINARY")
	candidate := os.Getenv("DW_CANDIDATE_BINARY")
	if oracle == "" || candidate == "" {
		t.Skip("external CLI parity requires both DW_ORACLE_BINARY (pinned Rust dw) and DW_CANDIDATE_BINARY (Go dw)")
	}
	for _, binary := range []struct{ name, executable string }{
		{name: "DW_ORACLE_BINARY", executable: oracle},
		{name: "DW_CANDIDATE_BINARY", executable: candidate},
	} {
		info, err := os.Stat(binary.executable)
		if err != nil {
			t.Fatalf("%s=%q: %v", binary.name, binary.executable, err)
		}
		if info.IsDir() {
			t.Fatalf("%s=%q is a directory", binary.name, binary.executable)
		}
	}
	return oracle, candidate
}

func TestExternalCLIDifferentialAgainstPinnedRustOracle(t *testing.T) {
	oracle, candidate := differentialBinaries(t)
	fixtures, err := paritytest.LoadFixtures(filepath.Join("..", "..", "testdata", "oracle", "cli-cases.json"))
	if err != nil {
		t.Fatal(err)
	}
	for _, fixture := range fixtures {
		fixture := fixture
		t.Run(fixture.Name, func(t *testing.T) {
			oracleCommand := fixture.Command(oracle)
			candidateCommand := fixture.Command(candidate)
			oracleCommand.Timeout = 30 * time.Second
			candidateCommand.Timeout = 30 * time.Second
			if err := paritytest.Pair(context.Background(), t.TempDir(), fixture.Name, oracleCommand, candidateCommand, nil); err != nil {
				t.Fatal(err)
			}
		})
	}
}

func TestExternalConfigInitRefreshDifferential(t *testing.T) {
	oracle, candidate := differentialBinaries(t)
	commands := func(executable string) []paritytest.Command {
		return []paritytest.Command{
			{Executable: executable, Args: []string{"init", "--root", "${ROOT}", "--no-save"}, Timeout: 30 * time.Second},
			{Executable: executable, Args: []string{"refresh", "--root", "${ROOT}"}, Timeout: 30 * time.Second},
		}
	}
	if err := paritytest.PairSequence(context.Background(), t.TempDir(), t.Name(), commands(oracle), commands(candidate), nil); err != nil {
		t.Fatal(err)
	}
}
