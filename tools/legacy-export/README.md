# OmniSpot LiteDB export

This separate Windows utility reads the C# profile without writing to either database or overwriting an existing export. Close the C# app first so LiteDB has flushed its journal, then run:

```powershell
dotnet run --project .\tools\legacy-export\OmniSpot.LegacyExport.csproj -- --profile "$env:APPDATA\OmniSpot" --out .\omnispot-migration-v1.json
```

It exports configuration, ordered app bindings, all daily stats, app names, and Spotlight launch counts. Key values include both the WPF enum name and integer. Unknown enum values fail the export rather than silently dropping a binding. The output includes a content digest and a source identity derived from the database bytes. Keep the source databases and export file; neither is deleted by this tool.

Run the synthetic read-only and mapping self-check with:

```powershell
dotnet run --project .\tools\legacy-export\OmniSpot.LegacyExport.csproj -- --self-test
```

For a cross-language smoke test, `--self-test-export <temporary-file>` writes the synthetic bundle after checking its digest and source-file hashes.

This is a one-time compatibility tool; ordinary Rust startup does not invoke .NET or LiteDB. Rust accepts a validated bundle through `omnispot-rust --import-legacy <export.json>`. Import writes one atomic `legacy-import.json` marker into a pristine profile; the Rust settings, stats, app registry, and Spotlight usage readers load from it until their normal JSON files are written. The original databases and export remain untouched.
