use std::collections::HashSet;

use crate::openapi::{Components, SchemaOrRef};

/// Resolve a $ref path like "#/components/schemas/Pet" to the referenced schema.
pub fn resolve_ref<'a>(
    ref_path: &str,
    components: Option<&'a Components>,
) -> Option<&'a SchemaOrRef> {
    let parts: Vec<&str> = ref_path
        .trim_start_matches('#')
        .trim_start_matches('/')
        .split('/')
        .collect();

    match parts.as_slice() {
        ["components", "schemas", name] => components?.schemas.get(*name),
        _ => None,
    }
}

/// Detect circular references in a schema graph using DFS.
pub fn detect_circular_refs(
    schema: &SchemaOrRef,
    components: Option<&Components>,
    visited: &mut HashSet<String>,
) -> bool {
    match schema {
        SchemaOrRef::Ref { ref_path } => {
            if visited.contains(ref_path) {
                return true;
            }
            visited.insert(ref_path.clone());
            if let Some(resolved) = resolve_ref(ref_path, components) {
                let is_circular = detect_circular_refs(resolved, components, visited);
                visited.remove(ref_path);
                is_circular
            } else {
                visited.remove(ref_path);
                false
            }
        }
        SchemaOrRef::Schema(schema) => {
            for prop in schema.properties.values() {
                if detect_circular_refs(prop, components, visited) {
                    return true;
                }
            }
            if let Some(items) = &schema.items
                && detect_circular_refs(items, components, visited)
            {
                return true;
            }
            if let Some(all_of) = &schema.all_of {
                for s in all_of {
                    if detect_circular_refs(s, components, visited) {
                        return true;
                    }
                }
            }
            if let Some(one_of) = &schema.one_of {
                for s in one_of {
                    if detect_circular_refs(s, components, visited) {
                        return true;
                    }
                }
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn resolve_simple_ref() {
        let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Pet:
      type: object
      properties:
        name:
          type: string
"#;
        let spec = parse(yaml).unwrap();
        let result = resolve_ref("#/components/schemas/Pet", spec.components.as_ref());
        assert!(result.is_some());
    }

    #[test]
    fn resolve_non_component_ref_returns_none() {
        let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Pet:
      type: object
      properties:
        name:
          type: string
"#;
        let spec = parse(yaml).unwrap();
        // Non-standard $ref path should return None
        assert!(resolve_ref("#/paths/get", spec.components.as_ref()).is_none());
        assert!(resolve_ref("#/definitions/Pet", spec.components.as_ref()).is_none());
        assert!(resolve_ref("", spec.components.as_ref()).is_none());
    }

    #[test]
    fn detect_circular_ref_through_array_items() {
        let yaml = r##"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    TreeNode:
      type: object
      properties:
        value:
          type: string
        children:
          type: array
          items:
            $ref: "#/components/schemas/TreeNode"
"##;
        let spec = parse(yaml).unwrap();
        let schema = spec
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("TreeNode")
            .unwrap();
        let mut visited = HashSet::new();
        assert!(detect_circular_refs(
            schema,
            spec.components.as_ref(),
            &mut visited
        ));
    }

    #[test]
    fn detect_circular_ref_through_allof() {
        let yaml = r##"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Base:
      allOf:
        - type: object
          properties:
            name:
              type: string
        - $ref: "#/components/schemas/Base"
"##;
        let spec = parse(yaml).unwrap();
        let schema = spec
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("Base")
            .unwrap();
        let mut visited = HashSet::new();
        assert!(detect_circular_refs(
            schema,
            spec.components.as_ref(),
            &mut visited
        ));
    }

    #[test]
    fn detect_circular_ref_through_oneof() {
        let yaml = r##"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Expression:
      oneOf:
        - type: object
          properties:
            value:
              type: number
        - $ref: "#/components/schemas/Expression"
"##;
        let spec = parse(yaml).unwrap();
        let schema = spec
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("Expression")
            .unwrap();
        let mut visited = HashSet::new();
        assert!(detect_circular_refs(
            schema,
            spec.components.as_ref(),
            &mut visited
        ));
    }

    #[test]
    fn non_circular_ref_returns_false() {
        let yaml = r##"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Owner:
      type: object
      properties:
        name:
          type: string
    Pet:
      type: object
      properties:
        owner:
          $ref: "#/components/schemas/Owner"
"##;
        let spec = parse(yaml).unwrap();
        let schema = spec
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("Pet")
            .unwrap();
        let mut visited = HashSet::new();
        assert!(!detect_circular_refs(
            schema,
            spec.components.as_ref(),
            &mut visited
        ));
    }

    #[test]
    fn unresolvable_ref_returns_false() {
        let yaml = r##"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Broken:
      type: object
      properties:
        missing:
          $ref: "#/components/schemas/DoesNotExist"
"##;
        let spec = parse(yaml).unwrap();
        let schema = spec
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("Broken")
            .unwrap();
        let mut visited = HashSet::new();
        assert!(!detect_circular_refs(
            schema,
            spec.components.as_ref(),
            &mut visited
        ));
    }

    #[test]
    fn detect_self_reference() {
        let yaml = r##"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Node:
      type: object
      properties:
        child:
          $ref: "#/components/schemas/Node"
"##;
        let spec = parse(yaml).unwrap();
        let schema = spec
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("Node")
            .unwrap();
        let mut visited = HashSet::new();
        assert!(detect_circular_refs(
            schema,
            spec.components.as_ref(),
            &mut visited
        ));
    }
}
