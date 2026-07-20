package paritytest_test

import (
	"bytes"
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/wirejson"
)

func TestWireJSONRetainsOrderDuplicatesNullsAndNumberLexemes(t *testing.T) {
	input := []byte(`{"z":1.2300,"unknown":{"b":2,"a":1},"same":"first","nullable":null,"same":"last"}`)
	value, err := wirejson.Parse(input)
	if err != nil {
		t.Fatal(err)
	}
	compact, err := wirejson.Compact(value)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(compact, input) {
		t.Fatalf("ordered round trip changed bytes:\nwant: %s\n got: %s", input, compact)
	}
	members, ok := value.Members()
	if !ok {
		t.Fatal("root is not an object")
	}
	gotNames := make([]string, len(members))
	for index := range members {
		gotNames[index] = members[index].Name
	}
	if want := []string{"z", "unknown", "same", "nullable", "same"}; !reflect.DeepEqual(gotNames, want) {
		t.Fatalf("member order = %#v, want %#v", gotNames, want)
	}
	last, ok := value.Lookup("same")
	if !ok {
		t.Fatal("duplicate member is missing")
	}
	if got, _ := last.AsString(); got != "last" {
		t.Fatalf("Lookup(same) = %q, want last", got)
	}
	nullable, ok := value.Lookup("nullable")
	if !ok || !nullable.IsNull() {
		t.Fatalf("explicit null was not retained: %#v", nullable)
	}
}

func TestWireJSONMutationPreservesSiblingPosition(t *testing.T) {
	value, err := wirejson.Parse([]byte(`{"first":1,"extension":{"keep":true},"target":"old","last":4}`))
	if err != nil {
		t.Fatal(err)
	}
	if err := value.Set("target", wirejson.StringValue("new")); err != nil {
		t.Fatal(err)
	}
	if err := value.Set("appended", wirejson.BoolValue(false)); err != nil {
		t.Fatal(err)
	}
	got, err := wirejson.Compact(value)
	if err != nil {
		t.Fatal(err)
	}
	want := []byte(`{"first":1,"extension":{"keep":true},"target":"new","last":4,"appended":false}`)
	if !bytes.Equal(got, want) {
		t.Fatalf("mutation changed order:\nwant: %s\n got: %s", want, got)
	}
}

func TestWireJSONEncodingIsByteStableAcrossRepeatedRuns(t *testing.T) {
	value, err := wirejson.Parse([]byte(`{"fields":{"workItemId":"42","state":"Active"},"relations":[3,2,1],"null":null}`))
	if err != nil {
		t.Fatal(err)
	}
	first, err := wirejson.Pretty(value)
	if err != nil {
		t.Fatal(err)
	}
	for iteration := 0; iteration < 100; iteration++ {
		got, err := wirejson.Pretty(value)
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(got, first) {
			t.Fatalf("encoding %d is nondeterministic:\nfirst: %s\n got: %s", iteration, first, got)
		}
	}
	var stream bytes.Buffer
	if err := wirejson.EncodePretty(&stream, value); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(stream.Bytes(), append(append([]byte(nil), first...), '\n')) {
		t.Fatalf("stream output must add exactly one newline: %q", stream.Bytes())
	}
}
