# terraform-forge

Terraform provider code generator from OpenAPI specs.

Implements `iac_forge::Backend` for the `terraform-plugin-framework` Go SDK,
generating complete Go source files for resources, data sources, providers,
and acceptance test scaffolds.

Part of the pleme-io code-generation pipeline:

```
sekkei -> takumi -> openapi-forge / iac-forge -> backend renderers
```

## License

MIT — see [LICENSE](./LICENSE).
