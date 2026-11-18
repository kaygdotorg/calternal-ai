---
title: "appearance.patch"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Patch this person's appearance preferences

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "appearance.patch",
  "description": "Patch this person's appearance preferences",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "AppearanceAccent": {
      "description": "Where the accent color is obtained.",
      "oneOf": [
        {
          "description": "Derive it from the wallpaper.",
          "enum": [
            "wallpaper"
          ],
          "type": "string"
        },
        {
          "description": "Use the person's custom hex color.",
          "enum": [
            "custom"
          ],
          "type": "string"
        },
        {
          "description": "Use the selected preset's palette.",
          "enum": [
            "theme"
          ],
          "type": "string"
        }
      ]
    },
    "AppearanceFontsPatch": {
      "additionalProperties": false,
      "description": "Partial update of role selections.",
      "properties": {
        "brand": {
          "anyOf": [
            {
              "$ref": "#/definitions/FontChoice"
            },
            {
              "type": "null"
            }
          ],
          "description": "Wordmark typeface."
        },
        "chat": {
          "anyOf": [
            {
              "$ref": "#/definitions/FontChoice"
            },
            {
              "type": "null"
            }
          ],
          "description": "Conversation typeface."
        },
        "code": {
          "anyOf": [
            {
              "$ref": "#/definitions/FontChoice"
            },
            {
              "type": "null"
            }
          ],
          "description": "Code typeface."
        },
        "terminal": {
          "anyOf": [
            {
              "$ref": "#/definitions/FontChoice"
            },
            {
              "type": "null"
            }
          ],
          "description": "Terminal typeface."
        },
        "ui": {
          "anyOf": [
            {
              "$ref": "#/definitions/FontChoice"
            },
            {
              "type": "null"
            }
          ],
          "description": "Interface typeface."
        }
      },
      "type": "object"
    },
    "AppearanceMaterialsPatch": {
      "additionalProperties": false,
      "description": "Partial update of material switches.",
      "properties": {
        "blur": {
          "description": "Blur backgrounds.",
          "type": [
            "boolean",
            "null"
          ]
        },
        "dim": {
          "description": "Dim backgrounds.",
          "type": [
            "boolean",
            "null"
          ]
        },
        "tint": {
          "description": "Tint backgrounds.",
          "type": [
            "boolean",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "FontChoice": {
      "additionalProperties": false,
      "description": "A role's selected typeface. Bundled and System faces omit the library ID.",
      "properties": {
        "family": {
          "description": "Display family name.",
          "type": "string"
        },
        "id": {
          "description": "Opaque library ID returned by catalogue or list.",
          "type": [
            "string",
            "null"
          ]
        },
        "source": {
          "allOf": [
            {
              "$ref": "#/definitions/FontSource"
            }
          ],
          "description": "Catalogue, upload or device source."
        }
      },
      "required": [
        "family",
        "source"
      ],
      "type": "object"
    },
    "FontSource": {
      "description": "Available catalogue and local font sources.",
      "oneOf": [
        {
          "description": "Google Fonts.",
          "enum": [
            "google"
          ],
          "type": "string"
        },
        {
          "description": "Fontsource.",
          "enum": [
            "fontsource"
          ],
          "type": "string"
        },
        {
          "description": "Fontshare.",
          "enum": [
            "fontshare"
          ],
          "type": "string"
        },
        {
          "description": "An account's own file.",
          "enum": [
            "upload"
          ],
          "type": "string"
        },
        {
          "description": "The device's system typeface.",
          "enum": [
            "system"
          ],
          "type": "string"
        }
      ]
    }
  },
  "description": "Apply only supplied fields. Explicit null clears nullable fields.",
  "properties": {
    "accent": {
      "anyOf": [
        {
          "$ref": "#/definitions/AppearanceAccent"
        },
        {
          "type": "null"
        }
      ],
      "description": "Accent color policy."
    },
    "accentColor": {
      "description": "Custom accent color in #rrggbb form.",
      "type": [
        "string",
        "null"
      ]
    },
    "brand": {
      "description": "Sidebar wordmark text.",
      "type": [
        "string",
        "null"
      ]
    },
    "fonts": {
      "anyOf": [
        {
          "$ref": "#/definitions/AppearanceFontsPatch"
        },
        {
          "type": "null"
        }
      ],
      "description": "Typeface selection per role."
    },
    "materials": {
      "anyOf": [
        {
          "$ref": "#/definitions/AppearanceMaterialsPatch"
        },
        {
          "type": "null"
        }
      ],
      "description": "Material effects."
    },
    "motion": {
      "description": "Enable motion when the device permits it.",
      "type": [
        "boolean",
        "null"
      ]
    },
    "presetId": {
      "description": "Last applied look, or null.",
      "type": [
        "string",
        "null"
      ]
    },
    "streamText": {
      "description": "Show streamed conversation text.",
      "type": [
        "boolean",
        "null"
      ]
    },
    "themeId": {
      "description": "Palette preset, or null for the built-in palette.",
      "type": [
        "string",
        "null"
      ]
    },
    "wallpaperId": {
      "description": "Wallpaper library ID, auto for the default, or null for none.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "title": "AppearancePatch",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "AppearanceAccent": {
      "description": "Where the accent color is obtained.",
      "oneOf": [
        {
          "description": "Derive it from the wallpaper.",
          "enum": [
            "wallpaper"
          ],
          "type": "string"
        },
        {
          "description": "Use the person's custom hex color.",
          "enum": [
            "custom"
          ],
          "type": "string"
        },
        {
          "description": "Use the selected preset's palette.",
          "enum": [
            "theme"
          ],
          "type": "string"
        }
      ]
    },
    "AppearanceFonts": {
      "additionalProperties": false,
      "description": "The account's five font selections.",
      "properties": {
        "brand": {
          "allOf": [
            {
              "$ref": "#/definitions/FontChoice"
            }
          ],
          "description": "Wordmark typeface."
        },
        "chat": {
          "allOf": [
            {
              "$ref": "#/definitions/FontChoice"
            }
          ],
          "description": "Conversation typeface."
        },
        "code": {
          "allOf": [
            {
              "$ref": "#/definitions/FontChoice"
            }
          ],
          "description": "Code typeface."
        },
        "terminal": {
          "allOf": [
            {
              "$ref": "#/definitions/FontChoice"
            }
          ],
          "description": "Terminal typeface."
        },
        "ui": {
          "allOf": [
            {
              "$ref": "#/definitions/FontChoice"
            }
          ],
          "description": "Interface typeface."
        }
      },
      "required": [
        "brand",
        "chat",
        "code",
        "terminal",
        "ui"
      ],
      "type": "object"
    },
    "AppearanceMaterials": {
      "additionalProperties": false,
      "description": "The account's material effects.",
      "properties": {
        "blur": {
          "description": "Blur backgrounds.",
          "type": "boolean"
        },
        "dim": {
          "description": "Dim backgrounds.",
          "type": "boolean"
        },
        "tint": {
          "description": "Tint backgrounds.",
          "type": "boolean"
        }
      },
      "required": [
        "blur",
        "dim",
        "tint"
      ],
      "type": "object"
    },
    "FontChoice": {
      "additionalProperties": false,
      "description": "A role's selected typeface. Bundled and System faces omit the library ID.",
      "properties": {
        "family": {
          "description": "Display family name.",
          "type": "string"
        },
        "id": {
          "description": "Opaque library ID returned by catalogue or list.",
          "type": [
            "string",
            "null"
          ]
        },
        "source": {
          "allOf": [
            {
              "$ref": "#/definitions/FontSource"
            }
          ],
          "description": "Catalogue, upload or device source."
        }
      },
      "required": [
        "family",
        "source"
      ],
      "type": "object"
    },
    "FontSource": {
      "description": "Available catalogue and local font sources.",
      "oneOf": [
        {
          "description": "Google Fonts.",
          "enum": [
            "google"
          ],
          "type": "string"
        },
        {
          "description": "Fontsource.",
          "enum": [
            "fontsource"
          ],
          "type": "string"
        },
        {
          "description": "Fontshare.",
          "enum": [
            "fontshare"
          ],
          "type": "string"
        },
        {
          "description": "An account's own file.",
          "enum": [
            "upload"
          ],
          "type": "string"
        },
        {
          "description": "The device's system typeface.",
          "enum": [
            "system"
          ],
          "type": "string"
        }
      ]
    }
  },
  "description": "The saved account appearance. A client overlays its local device settings for first paint.",
  "properties": {
    "accent": {
      "allOf": [
        {
          "$ref": "#/definitions/AppearanceAccent"
        }
      ],
      "description": "Accent color policy."
    },
    "accentColor": {
      "description": "Custom accent color in #rrggbb form.",
      "type": "string"
    },
    "brand": {
      "description": "Sidebar wordmark text.",
      "type": "string"
    },
    "fonts": {
      "allOf": [
        {
          "$ref": "#/definitions/AppearanceFonts"
        }
      ],
      "description": "Typeface selection per role."
    },
    "materials": {
      "allOf": [
        {
          "$ref": "#/definitions/AppearanceMaterials"
        }
      ],
      "description": "Material effects."
    },
    "motion": {
      "description": "Enable motion when the device permits it.",
      "type": "boolean"
    },
    "presetId": {
      "description": "Last applied look, or null.",
      "type": [
        "string",
        "null"
      ]
    },
    "streamText": {
      "description": "Show streamed conversation text.",
      "type": "boolean"
    },
    "themeId": {
      "description": "Palette preset, or null for the built-in palette.",
      "type": [
        "string",
        "null"
      ]
    },
    "wallpaperId": {
      "description": "Wallpaper library ID, auto for the default, or null for none.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "accent",
    "accentColor",
    "brand",
    "fonts",
    "materials",
    "motion",
    "streamText"
  ],
  "title": "AppearancePreferences",
  "type": "object"
}
```
