// Generated from pumas-rpc contract.rs; SHA256 743d700347e5480492c031c68383e64e6587d6329b60ffd58a3bde6e785f2e6a. DO NOT EDIT.
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function __require() {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

// node_modules/.pnpm/ajv@8.20.0/node_modules/ajv/dist/runtime/ucs2length.js
var require_ucs2length = __commonJS({
  "node_modules/.pnpm/ajv@8.20.0/node_modules/ajv/dist/runtime/ucs2length.js"(exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    function ucs2length(str) {
      const len = str.length;
      let length = 0;
      let pos = 0;
      let value;
      while (pos < len) {
        length++;
        value = str.charCodeAt(pos++);
        if (value >= 55296 && value <= 56319 && pos < len) {
          value = str.charCodeAt(pos);
          if ((value & 64512) === 56320)
            pos++;
        }
      }
      return length;
    }
    exports.default = ucs2length;
    ucs2length.code = 'require("ajv/dist/runtime/ucs2length").default';
  }
});

// node_modules/.pnpm/fast-deep-equal@3.1.3/node_modules/fast-deep-equal/index.js
var require_fast_deep_equal = __commonJS({
  "node_modules/.pnpm/fast-deep-equal@3.1.3/node_modules/fast-deep-equal/index.js"(exports, module) {
    "use strict";
    module.exports = function equal(a, b) {
      if (a === b) return true;
      if (a && b && typeof a == "object" && typeof b == "object") {
        if (a.constructor !== b.constructor) return false;
        var length, i, keys;
        if (Array.isArray(a)) {
          length = a.length;
          if (length != b.length) return false;
          for (i = length; i-- !== 0; )
            if (!equal(a[i], b[i])) return false;
          return true;
        }
        if (a.constructor === RegExp) return a.source === b.source && a.flags === b.flags;
        if (a.valueOf !== Object.prototype.valueOf) return a.valueOf() === b.valueOf();
        if (a.toString !== Object.prototype.toString) return a.toString() === b.toString();
        keys = Object.keys(a);
        length = keys.length;
        if (length !== Object.keys(b).length) return false;
        for (i = length; i-- !== 0; )
          if (!Object.prototype.hasOwnProperty.call(b, keys[i])) return false;
        for (i = length; i-- !== 0; ) {
          var key = keys[i];
          if (!equal(a[key], b[key])) return false;
        }
        return true;
      }
      return a !== a && b !== b;
    };
  }
});

// node_modules/.pnpm/ajv@8.20.0/node_modules/ajv/dist/runtime/equal.js
var require_equal = __commonJS({
  "node_modules/.pnpm/ajv@8.20.0/node_modules/ajv/dist/runtime/equal.js"(exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    var equal = require_fast_deep_equal();
    equal.code = 'require("ajv/dist/runtime/equal").default';
    exports.default = equal;
  }
});

// electron/desktop-contract.validators.js
var validateCatalogSearchOutcome = validate10;
var schema12 = { "additionalProperties": false, "properties": { "artifact": { "$ref": "#/definitions/CatalogArtifactState" }, "dependencyCount": { "maximum": 512, "minimum": 0, "type": "integer" }, "displayDate": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "displayName": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "format": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "id": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "integrity": { "$ref": "#/definitions/CatalogIntegrityState" }, "modelDir": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "modelType": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "quantization": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "relatedAvailable": { "type": "boolean" }, "sizeBytes": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" } }, "pumasCatalogRow": true, "required": ["id", "modelDir", "displayName", "modelType", "dependencyCount", "relatedAvailable", "artifact", "integrity"], "type": "object" };
var func2 = Object.prototype.hasOwnProperty;
var func4 = require_ucs2length().default;
var schema14 = { "enum": ["part_file_present", "expected_files_missing"], "type": "string" };
var func0 = require_equal().default;
var pattern0 = new RegExp("^v1:[0-9a-f]{64}$", "u");
var pattern1 = new RegExp("^(?!.*(?:--|\\.\\.))(?!.*\\.[gG][iI][tT]$)[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?/[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?$", "u");
var pattern2 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern3 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern4 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate12(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.state === void 0 && (missing0 = "state")) {
        const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (!(key0 === "state")) {
            const err1 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err1];
            } else {
              vErrors.push(err1);
            }
            errors++;
            break;
          }
        }
        if (_errs3 === errors) {
          if (data.state !== void 0) {
            let data0 = data.state;
            if (typeof data0 !== "string") {
              const err2 = { instancePath: instancePath + "/state", schemaPath: "#/oneOf/0/properties/state/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("complete" !== data0) {
              const err3 = { instancePath: instancePath + "/state", schemaPath: "#/oneOf/0/properties/state/const", keyword: "const", params: { allowedValue: "complete" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
          }
        }
      }
    } else {
      const err4 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs6 = errors;
  if (errors === _errs6) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing1;
      if (data.state === void 0 && (missing1 = "state") || data.reasons === void 0 && (missing1 = "reasons")) {
        const err5 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      } else {
        const _errs8 = errors;
        for (const key1 in data) {
          if (!(key1 === "downloadProgressFraction" || key1 === "reasons" || key1 === "recovery" || key1 === "state")) {
            const err6 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err6];
            } else {
              vErrors.push(err6);
            }
            errors++;
            break;
          }
        }
        if (_errs8 === errors) {
          if (data.downloadProgressFraction !== void 0) {
            let data1 = data.downloadProgressFraction;
            const _errs9 = errors;
            if (errors === _errs9) {
              if (typeof data1 == "number" && isFinite(data1)) {
                if (data1 > 17976931348623157e292 || isNaN(data1)) {
                  const err7 = { instancePath: instancePath + "/downloadProgressFraction", schemaPath: "#/oneOf/1/properties/downloadProgressFraction/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                } else {
                  if (data1 < 0 || isNaN(data1)) {
                    const err8 = { instancePath: instancePath + "/downloadProgressFraction", schemaPath: "#/oneOf/1/properties/downloadProgressFraction/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                    if (vErrors === null) {
                      vErrors = [err8];
                    } else {
                      vErrors.push(err8);
                    }
                    errors++;
                  } else {
                    if (data1 >= 1 || isNaN(data1)) {
                      const err9 = { instancePath: instancePath + "/downloadProgressFraction", schemaPath: "#/oneOf/1/properties/downloadProgressFraction/exclusiveMaximum", keyword: "exclusiveMaximum", params: { comparison: "<", limit: 1 }, message: "must be < 1" };
                      if (vErrors === null) {
                        vErrors = [err9];
                      } else {
                        vErrors.push(err9);
                      }
                      errors++;
                    }
                  }
                }
              } else {
                const err10 = { instancePath: instancePath + "/downloadProgressFraction", schemaPath: "#/oneOf/1/properties/downloadProgressFraction/type", keyword: "type", params: { type: "number" }, message: "must be number" };
                if (vErrors === null) {
                  vErrors = [err10];
                } else {
                  vErrors.push(err10);
                }
                errors++;
              }
            }
            var valid2 = _errs9 === errors;
          } else {
            var valid2 = true;
          }
          if (valid2) {
            if (data.reasons !== void 0) {
              let data2 = data.reasons;
              const _errs11 = errors;
              if (errors === _errs11) {
                if (Array.isArray(data2)) {
                  if (data2.length > 2) {
                    const err11 = { instancePath: instancePath + "/reasons", schemaPath: "#/oneOf/1/properties/reasons/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
                    if (vErrors === null) {
                      vErrors = [err11];
                    } else {
                      vErrors.push(err11);
                    }
                    errors++;
                  } else {
                    if (data2.length < 1) {
                      const err12 = { instancePath: instancePath + "/reasons", schemaPath: "#/oneOf/1/properties/reasons/minItems", keyword: "minItems", params: { limit: 1 }, message: "must NOT have fewer than 1 items" };
                      if (vErrors === null) {
                        vErrors = [err12];
                      } else {
                        vErrors.push(err12);
                      }
                      errors++;
                    } else {
                      var valid3 = true;
                      const len0 = data2.length;
                      for (let i0 = 0; i0 < len0; i0++) {
                        let data3 = data2[i0];
                        const _errs13 = errors;
                        if (typeof data3 !== "string") {
                          const err13 = { instancePath: instancePath + "/reasons/" + i0, schemaPath: "#/definitions/CatalogPartialReason/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                          if (vErrors === null) {
                            vErrors = [err13];
                          } else {
                            vErrors.push(err13);
                          }
                          errors++;
                        }
                        if (!(data3 === "part_file_present" || data3 === "expected_files_missing")) {
                          const err14 = { instancePath: instancePath + "/reasons/" + i0, schemaPath: "#/definitions/CatalogPartialReason/enum", keyword: "enum", params: { allowedValues: schema14.enum }, message: "must be equal to one of the allowed values" };
                          if (vErrors === null) {
                            vErrors = [err14];
                          } else {
                            vErrors.push(err14);
                          }
                          errors++;
                        }
                        var valid3 = _errs13 === errors;
                        if (!valid3) {
                          break;
                        }
                      }
                      if (valid3) {
                        let i1 = data2.length;
                        let j0;
                        if (i1 > 1) {
                          outer0: for (; i1--; ) {
                            for (j0 = i1; j0--; ) {
                              if (func0(data2[i1], data2[j0])) {
                                const err15 = { instancePath: instancePath + "/reasons", schemaPath: "#/oneOf/1/properties/reasons/uniqueItems", keyword: "uniqueItems", params: { i: i1, j: j0 }, message: "must NOT have duplicate items (items ## " + j0 + " and " + i1 + " are identical)" };
                                if (vErrors === null) {
                                  vErrors = [err15];
                                } else {
                                  vErrors.push(err15);
                                }
                                errors++;
                                break outer0;
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                } else {
                  const err16 = { instancePath: instancePath + "/reasons", schemaPath: "#/oneOf/1/properties/reasons/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                  if (vErrors === null) {
                    vErrors = [err16];
                  } else {
                    vErrors.push(err16);
                  }
                  errors++;
                }
              }
              var valid2 = _errs11 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.recovery !== void 0) {
                let data4 = data.recovery;
                const _errs16 = errors;
                const _errs17 = errors;
                if (errors === _errs17) {
                  if (data4 && typeof data4 == "object" && !Array.isArray(data4)) {
                    let missing2;
                    if (data4.recoveryToken === void 0 && (missing2 = "recoveryToken") || data4.repoId === void 0 && (missing2 = "repoId")) {
                      const err17 = { instancePath: instancePath + "/recovery", schemaPath: "#/definitions/CatalogRecoveryIdentity/required", keyword: "required", params: { missingProperty: missing2 }, message: "must have required property '" + missing2 + "'" };
                      if (vErrors === null) {
                        vErrors = [err17];
                      } else {
                        vErrors.push(err17);
                      }
                      errors++;
                    } else {
                      const _errs19 = errors;
                      for (const key2 in data4) {
                        if (!(key2 === "recoveryToken" || key2 === "repoId" || key2 === "selectedArtifactFiles" || key2 === "selectedArtifactId" || key2 === "selectedArtifactQuant")) {
                          const err18 = { instancePath: instancePath + "/recovery", schemaPath: "#/definitions/CatalogRecoveryIdentity/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
                          if (vErrors === null) {
                            vErrors = [err18];
                          } else {
                            vErrors.push(err18);
                          }
                          errors++;
                          break;
                        }
                      }
                      if (_errs19 === errors) {
                        if (data4.recoveryToken !== void 0) {
                          let data5 = data4.recoveryToken;
                          const _errs20 = errors;
                          if (errors === _errs20) {
                            if (typeof data5 === "string") {
                              if (!pattern0.test(data5)) {
                                const err19 = { instancePath: instancePath + "/recovery/recoveryToken", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/recoveryToken/pattern", keyword: "pattern", params: { pattern: "^v1:[0-9a-f]{64}$" }, message: 'must match pattern "^v1:[0-9a-f]{64}$"' };
                                if (vErrors === null) {
                                  vErrors = [err19];
                                } else {
                                  vErrors.push(err19);
                                }
                                errors++;
                              }
                            } else {
                              const err20 = { instancePath: instancePath + "/recovery/recoveryToken", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/recoveryToken/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                              if (vErrors === null) {
                                vErrors = [err20];
                              } else {
                                vErrors.push(err20);
                              }
                              errors++;
                            }
                          }
                          var valid7 = _errs20 === errors;
                        } else {
                          var valid7 = true;
                        }
                        if (valid7) {
                          if (data4.repoId !== void 0) {
                            let data6 = data4.repoId;
                            const _errs22 = errors;
                            if (errors === _errs22) {
                              if (typeof data6 === "string") {
                                if (func4(data6) > 96) {
                                  const err21 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/maxLength", keyword: "maxLength", params: { limit: 96 }, message: "must NOT have more than 96 characters" };
                                  if (vErrors === null) {
                                    vErrors = [err21];
                                  } else {
                                    vErrors.push(err21);
                                  }
                                  errors++;
                                } else {
                                  if (!pattern1.test(data6)) {
                                    const err22 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/pattern", keyword: "pattern", params: { pattern: "^(?!.*(?:--|\\.\\.))(?!.*\\.[gG][iI][tT]$)[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?/[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?$" }, message: 'must match pattern "^(?!.*(?:--|\\.\\.))(?!.*\\.[gG][iI][tT]$)[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?/[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?$"' };
                                    if (vErrors === null) {
                                      vErrors = [err22];
                                    } else {
                                      vErrors.push(err22);
                                    }
                                    errors++;
                                  } else {
                                    if (data6.length === 0 || pattern2.test(data6)) {
                                      const err23 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' };
                                      if (vErrors === null) {
                                        vErrors = [err23];
                                      } else {
                                        vErrors.push(err23);
                                      }
                                      errors++;
                                    } else {
                                      if (encodeURIComponent(data6).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        const err24 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' };
                                        if (vErrors === null) {
                                          vErrors = [err24];
                                        } else {
                                          vErrors.push(err24);
                                        }
                                        errors++;
                                      }
                                    }
                                  }
                                }
                              } else {
                                const err25 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                if (vErrors === null) {
                                  vErrors = [err25];
                                } else {
                                  vErrors.push(err25);
                                }
                                errors++;
                              }
                            }
                            var valid7 = _errs22 === errors;
                          } else {
                            var valid7 = true;
                          }
                          if (valid7) {
                            if (data4.selectedArtifactFiles !== void 0) {
                              let data7 = data4.selectedArtifactFiles;
                              const _errs24 = errors;
                              if (errors === _errs24) {
                                if (Array.isArray(data7)) {
                                  if (data7.length > 512) {
                                    const err26 = { instancePath: instancePath + "/recovery/selectedArtifactFiles", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/maxItems", keyword: "maxItems", params: { limit: 512 }, message: "must NOT have more than 512 items" };
                                    if (vErrors === null) {
                                      vErrors = [err26];
                                    } else {
                                      vErrors.push(err26);
                                    }
                                    errors++;
                                  } else {
                                    if (data7.length < 1) {
                                      const err27 = { instancePath: instancePath + "/recovery/selectedArtifactFiles", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/minItems", keyword: "minItems", params: { limit: 1 }, message: "must NOT have fewer than 1 items" };
                                      if (vErrors === null) {
                                        vErrors = [err27];
                                      } else {
                                        vErrors.push(err27);
                                      }
                                      errors++;
                                    } else {
                                      var valid8 = true;
                                      const len1 = data7.length;
                                      for (let i2 = 0; i2 < len1; i2++) {
                                        let data8 = data7[i2];
                                        const _errs26 = errors;
                                        if (errors === _errs26) {
                                          if (typeof data8 === "string") {
                                            if (data8.length === 0 || data8.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(data8) || Array.from(data8).some((letter) => letter.codePointAt(0) < 32 || letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159) || data8.split("/").some((component) => {
                                              const stem = component.split(".")[0].replace(/[a-z]/g, (letter) => letter.toUpperCase());
                                              return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g, "x").length > 255 || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
                                            })) {
                                              const err28 = { instancePath: instancePath + "/recovery/selectedArtifactFiles/" + i2, schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/items/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' };
                                              if (vErrors === null) {
                                                vErrors = [err28];
                                              } else {
                                                vErrors.push(err28);
                                              }
                                              errors++;
                                            } else {
                                              if (encodeURIComponent(data8).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                                const err29 = { instancePath: instancePath + "/recovery/selectedArtifactFiles/" + i2, schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/items/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' };
                                                if (vErrors === null) {
                                                  vErrors = [err29];
                                                } else {
                                                  vErrors.push(err29);
                                                }
                                                errors++;
                                              }
                                            }
                                          } else {
                                            const err30 = { instancePath: instancePath + "/recovery/selectedArtifactFiles/" + i2, schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/items/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                            if (vErrors === null) {
                                              vErrors = [err30];
                                            } else {
                                              vErrors.push(err30);
                                            }
                                            errors++;
                                          }
                                        }
                                        var valid8 = _errs26 === errors;
                                        if (!valid8) {
                                          break;
                                        }
                                      }
                                      if (valid8) {
                                        let i3 = data7.length;
                                        let j1;
                                        if (i3 > 1) {
                                          const indices0 = {};
                                          for (; i3--; ) {
                                            let item0 = data7[i3];
                                            if (typeof item0 !== "string") {
                                              continue;
                                            }
                                            if (typeof indices0[item0] == "number") {
                                              j1 = indices0[item0];
                                              const err31 = { instancePath: instancePath + "/recovery/selectedArtifactFiles", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/uniqueItems", keyword: "uniqueItems", params: { i: i3, j: j1 }, message: "must NOT have duplicate items (items ## " + j1 + " and " + i3 + " are identical)" };
                                              if (vErrors === null) {
                                                vErrors = [err31];
                                              } else {
                                                vErrors.push(err31);
                                              }
                                              errors++;
                                              break;
                                            }
                                            indices0[item0] = i3;
                                          }
                                        }
                                      }
                                    }
                                  }
                                } else {
                                  const err32 = { instancePath: instancePath + "/recovery/selectedArtifactFiles", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                                  if (vErrors === null) {
                                    vErrors = [err32];
                                  } else {
                                    vErrors.push(err32);
                                  }
                                  errors++;
                                }
                              }
                              var valid7 = _errs24 === errors;
                            } else {
                              var valid7 = true;
                            }
                            if (valid7) {
                              if (data4.selectedArtifactId !== void 0) {
                                let data9 = data4.selectedArtifactId;
                                const _errs28 = errors;
                                if (errors === _errs28) {
                                  if (typeof data9 === "string") {
                                    if (data9.length === 0 || pattern3.test(data9)) {
                                      const err33 = { instancePath: instancePath + "/recovery/selectedArtifactId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactId/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' };
                                      if (vErrors === null) {
                                        vErrors = [err33];
                                      } else {
                                        vErrors.push(err33);
                                      }
                                      errors++;
                                    } else {
                                      if (encodeURIComponent(data9).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        const err34 = { instancePath: instancePath + "/recovery/selectedArtifactId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' };
                                        if (vErrors === null) {
                                          vErrors = [err34];
                                        } else {
                                          vErrors.push(err34);
                                        }
                                        errors++;
                                      }
                                    }
                                  } else {
                                    const err35 = { instancePath: instancePath + "/recovery/selectedArtifactId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                    if (vErrors === null) {
                                      vErrors = [err35];
                                    } else {
                                      vErrors.push(err35);
                                    }
                                    errors++;
                                  }
                                }
                                var valid7 = _errs28 === errors;
                              } else {
                                var valid7 = true;
                              }
                              if (valid7) {
                                if (data4.selectedArtifactQuant !== void 0) {
                                  let data10 = data4.selectedArtifactQuant;
                                  const _errs30 = errors;
                                  if (errors === _errs30) {
                                    if (typeof data10 === "string") {
                                      if (data10.length === 0 || pattern4.test(data10)) {
                                        const err36 = { instancePath: instancePath + "/recovery/selectedArtifactQuant", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactQuant/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' };
                                        if (vErrors === null) {
                                          vErrors = [err36];
                                        } else {
                                          vErrors.push(err36);
                                        }
                                        errors++;
                                      } else {
                                        if (encodeURIComponent(data10).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                          const err37 = { instancePath: instancePath + "/recovery/selectedArtifactQuant", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactQuant/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' };
                                          if (vErrors === null) {
                                            vErrors = [err37];
                                          } else {
                                            vErrors.push(err37);
                                          }
                                          errors++;
                                        }
                                      }
                                    } else {
                                      const err38 = { instancePath: instancePath + "/recovery/selectedArtifactQuant", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactQuant/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                      if (vErrors === null) {
                                        vErrors = [err38];
                                      } else {
                                        vErrors.push(err38);
                                      }
                                      errors++;
                                    }
                                  }
                                  var valid7 = _errs30 === errors;
                                } else {
                                  var valid7 = true;
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  } else {
                    const err39 = { instancePath: instancePath + "/recovery", schemaPath: "#/definitions/CatalogRecoveryIdentity/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                    if (vErrors === null) {
                      vErrors = [err39];
                    } else {
                      vErrors.push(err39);
                    }
                    errors++;
                  }
                }
                var valid2 = _errs16 === errors;
              } else {
                var valid2 = true;
              }
              if (valid2) {
                if (data.state !== void 0) {
                  let data11 = data.state;
                  const _errs32 = errors;
                  if (typeof data11 !== "string") {
                    const err40 = { instancePath: instancePath + "/state", schemaPath: "#/oneOf/1/properties/state/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err40];
                    } else {
                      vErrors.push(err40);
                    }
                    errors++;
                  }
                  if ("partial" !== data11) {
                    const err41 = { instancePath: instancePath + "/state", schemaPath: "#/oneOf/1/properties/state/const", keyword: "const", params: { allowedValue: "partial" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err41];
                    } else {
                      vErrors.push(err41);
                    }
                    errors++;
                  }
                  var valid2 = _errs32 === errors;
                } else {
                  var valid2 = true;
                }
              }
            }
          }
        }
      }
    } else {
      const err42 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err42];
      } else {
        vErrors.push(err42);
      }
      errors++;
    }
  }
  var _valid0 = _errs6 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
  }
  if (!valid0) {
    const err43 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err43];
    } else {
      vErrors.push(err43);
    }
    errors++;
    validate12.errors = vErrors;
    return false;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate12.errors = vErrors;
  return errors === 0;
}
var pattern5 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern6 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern7 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern8 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern9 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern10 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern11 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate11(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.id === void 0 && (missing0 = "id") || data.modelDir === void 0 && (missing0 = "modelDir") || data.displayName === void 0 && (missing0 = "displayName") || data.modelType === void 0 && (missing0 = "modelType") || data.dependencyCount === void 0 && (missing0 = "dependencyCount") || data.relatedAvailable === void 0 && (missing0 = "relatedAvailable") || data.artifact === void 0 && (missing0 = "artifact") || data.integrity === void 0 && (missing0 = "integrity")) {
        validate11.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema12.properties, key0)) {
            validate11.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.artifact !== void 0) {
            const _errs2 = errors;
            if (!validate12(data.artifact, { instancePath: instancePath + "/artifact", parentData: data, parentDataProperty: "artifact", rootData })) {
              vErrors = vErrors === null ? validate12.errors : vErrors.concat(validate12.errors);
              errors = vErrors.length;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.dependencyCount !== void 0) {
              let data1 = data.dependencyCount;
              const _errs3 = errors;
              if (!(typeof data1 == "number" && (!(data1 % 1) && !isNaN(data1)) && isFinite(data1))) {
                validate11.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs3) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 512 || isNaN(data1)) {
                    validate11.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 512 }, message: "must be <= 512" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate11.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                      return false;
                    }
                  }
                }
              }
              var valid0 = _errs3 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.displayDate !== void 0) {
                let data2 = data.displayDate;
                const _errs5 = errors;
                if (errors === _errs5) {
                  if (typeof data2 === "string") {
                    if (func4(data2) < 1) {
                      validate11.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                      return false;
                    } else {
                      if (data2.length === 0 || pattern5.test(data2)) {
                        validate11.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                        return false;
                      } else {
                        if (encodeURIComponent(data2).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                          validate11.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate11.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                }
                var valid0 = _errs5 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.displayName !== void 0) {
                  let data3 = data.displayName;
                  const _errs7 = errors;
                  if (errors === _errs7) {
                    if (typeof data3 === "string") {
                      if (func4(data3) < 1) {
                        validate11.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                        return false;
                      } else {
                        if (data3.length === 0 || pattern6.test(data3)) {
                          validate11.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data3).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate11.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate11.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                      return false;
                    }
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.format !== void 0) {
                    let data4 = data.format;
                    const _errs9 = errors;
                    if (errors === _errs9) {
                      if (typeof data4 === "string") {
                        if (func4(data4) < 1) {
                          validate11.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                          return false;
                        } else {
                          if (data4.length === 0 || pattern7.test(data4)) {
                            validate11.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                            return false;
                          } else {
                            if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                              validate11.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate11.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                    }
                    var valid0 = _errs9 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.id !== void 0) {
                      let data5 = data.id;
                      const _errs11 = errors;
                      if (errors === _errs11) {
                        if (typeof data5 === "string") {
                          if (func4(data5) < 1) {
                            validate11.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                            return false;
                          } else {
                            if (data5.length === 0 || pattern8.test(data5)) {
                              validate11.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                              return false;
                            } else {
                              if (encodeURIComponent(data5).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                validate11.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate11.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                          return false;
                        }
                      }
                      var valid0 = _errs11 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.integrity !== void 0) {
                        let data6 = data.integrity;
                        const _errs13 = errors;
                        const _errs15 = errors;
                        let valid2 = false;
                        let passing0 = null;
                        const _errs16 = errors;
                        if (errors === _errs16) {
                          if (data6 && typeof data6 == "object" && !Array.isArray(data6)) {
                            let missing1;
                            if (data6.state === void 0 && (missing1 = "state")) {
                              const err0 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
                              if (vErrors === null) {
                                vErrors = [err0];
                              } else {
                                vErrors.push(err0);
                              }
                              errors++;
                            } else {
                              const _errs18 = errors;
                              for (const key1 in data6) {
                                if (!(key1 === "state")) {
                                  const err1 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
                                  if (vErrors === null) {
                                    vErrors = [err1];
                                  } else {
                                    vErrors.push(err1);
                                  }
                                  errors++;
                                  break;
                                }
                              }
                              if (_errs18 === errors) {
                                if (data6.state !== void 0) {
                                  let data7 = data6.state;
                                  if (typeof data7 !== "string") {
                                    const err2 = { instancePath: instancePath + "/integrity/state", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/properties/state/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                    if (vErrors === null) {
                                      vErrors = [err2];
                                    } else {
                                      vErrors.push(err2);
                                    }
                                    errors++;
                                  }
                                  if ("clean" !== data7) {
                                    const err3 = { instancePath: instancePath + "/integrity/state", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/properties/state/const", keyword: "const", params: { allowedValue: "clean" }, message: "must be equal to constant" };
                                    if (vErrors === null) {
                                      vErrors = [err3];
                                    } else {
                                      vErrors.push(err3);
                                    }
                                    errors++;
                                  }
                                }
                              }
                            }
                          } else {
                            const err4 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                            if (vErrors === null) {
                              vErrors = [err4];
                            } else {
                              vErrors.push(err4);
                            }
                            errors++;
                          }
                        }
                        var _valid0 = _errs16 === errors;
                        if (_valid0) {
                          valid2 = true;
                          passing0 = 0;
                        }
                        const _errs21 = errors;
                        if (errors === _errs21) {
                          if (data6 && typeof data6 == "object" && !Array.isArray(data6)) {
                            let missing2;
                            if (data6.state === void 0 && (missing2 = "state") || data6.count === void 0 && (missing2 = "count") || data6.otherModelIds === void 0 && (missing2 = "otherModelIds")) {
                              const err5 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/required", keyword: "required", params: { missingProperty: missing2 }, message: "must have required property '" + missing2 + "'" };
                              if (vErrors === null) {
                                vErrors = [err5];
                              } else {
                                vErrors.push(err5);
                              }
                              errors++;
                            } else {
                              const _errs23 = errors;
                              for (const key2 in data6) {
                                if (!(key2 === "count" || key2 === "otherModelIds" || key2 === "state")) {
                                  const err6 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
                                  if (vErrors === null) {
                                    vErrors = [err6];
                                  } else {
                                    vErrors.push(err6);
                                  }
                                  errors++;
                                  break;
                                }
                              }
                              if (_errs23 === errors) {
                                if (data6.count !== void 0) {
                                  let data8 = data6.count;
                                  const _errs24 = errors;
                                  if (!(typeof data8 == "number" && (!(data8 % 1) && !isNaN(data8)) && isFinite(data8))) {
                                    const err7 = { instancePath: instancePath + "/integrity/count", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/count/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
                                    if (vErrors === null) {
                                      vErrors = [err7];
                                    } else {
                                      vErrors.push(err7);
                                    }
                                    errors++;
                                  }
                                  if (errors === _errs24) {
                                    if (typeof data8 == "number" && isFinite(data8)) {
                                      if (data8 > 4294967295 || isNaN(data8)) {
                                        const err8 = { instancePath: instancePath + "/integrity/count", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" };
                                        if (vErrors === null) {
                                          vErrors = [err8];
                                        } else {
                                          vErrors.push(err8);
                                        }
                                        errors++;
                                      } else {
                                        if (data8 < 0 || isNaN(data8)) {
                                          const err9 = { instancePath: instancePath + "/integrity/count", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                                          if (vErrors === null) {
                                            vErrors = [err9];
                                          } else {
                                            vErrors.push(err9);
                                          }
                                          errors++;
                                        }
                                      }
                                    }
                                  }
                                  var valid4 = _errs24 === errors;
                                } else {
                                  var valid4 = true;
                                }
                                if (valid4) {
                                  if (data6.otherModelIds !== void 0) {
                                    let data9 = data6.otherModelIds;
                                    const _errs26 = errors;
                                    if (errors === _errs26) {
                                      if (Array.isArray(data9)) {
                                        var valid5 = true;
                                        const len0 = data9.length;
                                        for (let i0 = 0; i0 < len0; i0++) {
                                          const _errs28 = errors;
                                          if (typeof data9[i0] !== "string") {
                                            const err10 = { instancePath: instancePath + "/integrity/otherModelIds/" + i0, schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/otherModelIds/items/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                            if (vErrors === null) {
                                              vErrors = [err10];
                                            } else {
                                              vErrors.push(err10);
                                            }
                                            errors++;
                                          }
                                          var valid5 = _errs28 === errors;
                                          if (!valid5) {
                                            break;
                                          }
                                        }
                                      } else {
                                        const err11 = { instancePath: instancePath + "/integrity/otherModelIds", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/otherModelIds/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                                        if (vErrors === null) {
                                          vErrors = [err11];
                                        } else {
                                          vErrors.push(err11);
                                        }
                                        errors++;
                                      }
                                    }
                                    var valid4 = _errs26 === errors;
                                  } else {
                                    var valid4 = true;
                                  }
                                  if (valid4) {
                                    if (data6.state !== void 0) {
                                      let data11 = data6.state;
                                      const _errs30 = errors;
                                      if (typeof data11 !== "string") {
                                        const err12 = { instancePath: instancePath + "/integrity/state", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/state/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                        if (vErrors === null) {
                                          vErrors = [err12];
                                        } else {
                                          vErrors.push(err12);
                                        }
                                        errors++;
                                      }
                                      if ("duplicate" !== data11) {
                                        const err13 = { instancePath: instancePath + "/integrity/state", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/state/const", keyword: "const", params: { allowedValue: "duplicate" }, message: "must be equal to constant" };
                                        if (vErrors === null) {
                                          vErrors = [err13];
                                        } else {
                                          vErrors.push(err13);
                                        }
                                        errors++;
                                      }
                                      var valid4 = _errs30 === errors;
                                    } else {
                                      var valid4 = true;
                                    }
                                  }
                                }
                              }
                            }
                          } else {
                            const err14 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                            if (vErrors === null) {
                              vErrors = [err14];
                            } else {
                              vErrors.push(err14);
                            }
                            errors++;
                          }
                        }
                        var _valid0 = _errs21 === errors;
                        if (_valid0 && valid2) {
                          valid2 = false;
                          passing0 = [passing0, 1];
                        } else {
                          if (_valid0) {
                            valid2 = true;
                            passing0 = 1;
                          }
                        }
                        if (!valid2) {
                          const err15 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
                          if (vErrors === null) {
                            vErrors = [err15];
                          } else {
                            vErrors.push(err15);
                          }
                          errors++;
                          validate11.errors = vErrors;
                          return false;
                        } else {
                          errors = _errs15;
                          if (vErrors !== null) {
                            if (_errs15) {
                              vErrors.length = _errs15;
                            } else {
                              vErrors = null;
                            }
                          }
                        }
                        var valid0 = _errs13 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.modelDir !== void 0) {
                          let data12 = data.modelDir;
                          const _errs32 = errors;
                          if (errors === _errs32) {
                            if (typeof data12 === "string") {
                              if (func4(data12) < 1) {
                                validate11.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                return false;
                              } else {
                                if (data12.length === 0 || pattern9.test(data12)) {
                                  validate11.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                  return false;
                                } else {
                                  if (encodeURIComponent(data12).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                    validate11.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                    return false;
                                  }
                                }
                              }
                            } else {
                              validate11.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                              return false;
                            }
                          }
                          var valid0 = _errs32 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.modelType !== void 0) {
                            let data13 = data.modelType;
                            const _errs34 = errors;
                            if (errors === _errs34) {
                              if (typeof data13 === "string") {
                                if (func4(data13) < 1) {
                                  validate11.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                  return false;
                                } else {
                                  if (data13.length === 0 || pattern10.test(data13)) {
                                    validate11.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                    return false;
                                  } else {
                                    if (encodeURIComponent(data13).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                      validate11.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                      return false;
                                    }
                                  }
                                }
                              } else {
                                validate11.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                return false;
                              }
                            }
                            var valid0 = _errs34 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.quantization !== void 0) {
                              let data14 = data.quantization;
                              const _errs36 = errors;
                              if (errors === _errs36) {
                                if (typeof data14 === "string") {
                                  if (func4(data14) < 1) {
                                    validate11.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                    return false;
                                  } else {
                                    if (data14.length === 0 || pattern11.test(data14)) {
                                      validate11.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                      return false;
                                    } else {
                                      if (encodeURIComponent(data14).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        validate11.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                        return false;
                                      }
                                    }
                                  }
                                } else {
                                  validate11.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                  return false;
                                }
                              }
                              var valid0 = _errs36 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.relatedAvailable !== void 0) {
                                const _errs38 = errors;
                                if (typeof data.relatedAvailable !== "boolean") {
                                  validate11.errors = [{ instancePath: instancePath + "/relatedAvailable", schemaPath: "#/properties/relatedAvailable/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                  return false;
                                }
                                var valid0 = _errs38 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.sizeBytes !== void 0) {
                                  let data16 = data.sizeBytes;
                                  const _errs40 = errors;
                                  if (!(typeof data16 == "number" && (!(data16 % 1) && !isNaN(data16)) && isFinite(data16))) {
                                    validate11.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                    return false;
                                  }
                                  if (errors === _errs40) {
                                    if (typeof data16 == "number" && isFinite(data16)) {
                                      if (data16 > 9007199254740991 || isNaN(data16)) {
                                        validate11.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                        return false;
                                      } else {
                                        if (data16 < 0 || isNaN(data16)) {
                                          validate11.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                          return false;
                                        }
                                      }
                                    }
                                  }
                                  var valid0 = _errs40 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.integrity?.state === "duplicate" && (!Array.isArray(data.integrity.otherModelIds) || data.integrity.count !== data.integrity.otherModelIds.length + 1 || data.integrity.count < 2 || data.integrity.otherModelIds.includes(data.id) || new Set(data.integrity.otherModelIds).size !== data.integrity.otherModelIds.length)) {
                                    validate11.errors = [{ instancePath, schemaPath: "#/pumasCatalogRow", keyword: "pumasCatalogRow", params: {}, message: 'must pass "pumasCatalogRow" keyword validation' }];
                                    return false;
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate11.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate11.errors = vErrors;
  return errors === 0;
}
function validate10(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.models === void 0 && (missing0 = "models") || data.total_count === void 0 && (missing0 = "total_count") || data.query_time_ms === void 0 && (missing0 = "query_time_ms") || data.query === void 0 && (missing0 = "query")) {
        validate10.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "models" || key0 === "query" || key0 === "query_time_ms" || key0 === "success" || key0 === "total_count")) {
            validate10.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.models !== void 0) {
            let data0 = data.models;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (!validate11(data0[i0], { instancePath: instancePath + "/models/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate11.errors : vErrors.concat(validate11.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate10.errors = [{ instancePath: instancePath + "/models", schemaPath: "#/properties/models/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.query !== void 0) {
              let data2 = data.query;
              const _errs5 = errors;
              if (errors === _errs5) {
                if (typeof data2 === "string") {
                  if (encodeURIComponent(data2).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                    validate10.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                } else {
                  validate10.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                  return false;
                }
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.query_time_ms !== void 0) {
                let data3 = data.query_time_ms;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data3 == "number" && isFinite(data3)) {
                    if (data3 > 17976931348623157e292 || isNaN(data3)) {
                      validate10.errors = [{ instancePath: instancePath + "/query_time_ms", schemaPath: "#/properties/query_time_ms/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                      return false;
                    } else {
                      if (data3 < 0 || isNaN(data3)) {
                        validate10.errors = [{ instancePath: instancePath + "/query_time_ms", schemaPath: "#/properties/query_time_ms/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                        return false;
                      }
                    }
                  } else {
                    validate10.errors = [{ instancePath: instancePath + "/query_time_ms", schemaPath: "#/properties/query_time_ms/type", keyword: "type", params: { type: "number" }, message: "must be number" }];
                    return false;
                  }
                }
                var valid0 = _errs7 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.success !== void 0) {
                  let data4 = data.success;
                  const _errs9 = errors;
                  if (typeof data4 !== "boolean") {
                    validate10.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                    return false;
                  }
                  if (true !== data4) {
                    validate10.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                    return false;
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.total_count !== void 0) {
                    let data5 = data.total_count;
                    const _errs11 = errors;
                    if (!(typeof data5 == "number" && (!(data5 % 1) && !isNaN(data5)) && isFinite(data5))) {
                      validate10.errors = [{ instancePath: instancePath + "/total_count", schemaPath: "#/properties/total_count/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                      return false;
                    }
                    if (errors === _errs11) {
                      if (typeof data5 == "number" && isFinite(data5)) {
                        if (data5 > 9007199254740991 || isNaN(data5)) {
                          validate10.errors = [{ instancePath: instancePath + "/total_count", schemaPath: "#/properties/total_count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                          return false;
                        } else {
                          if (data5 < 0 || isNaN(data5)) {
                            validate10.errors = [{ instancePath: instancePath + "/total_count", schemaPath: "#/properties/total_count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                            return false;
                          }
                        }
                      }
                    }
                    var valid0 = _errs11 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (!Array.isArray(data.models) || data.total_count < data.models.length || new Set(data.models.map((model) => model.id)).size !== data.models.length) {
                      validate10.errors = [{ instancePath, schemaPath: "#/pumasCatalogSearch", keyword: "pumasCatalogSearch", params: {}, message: 'must pass "pumasCatalogSearch" keyword validation' }];
                      return false;
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate10.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate10.errors = vErrors;
  return errors === 0;
}
var validateDownloadIdParams = validate15;
function validate15(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.download_id === void 0 && (missing0 = "download_id")) {
        validate15.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "download_id")) {
            validate15.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.download_id !== void 0) {
            let data0 = data.download_id;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (typeof data0 === "string") {
                if (func4(data0) < 1) {
                  validate15.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                  return false;
                } else {
                  if (encodeURIComponent(data0).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                    validate15.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate15.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
            }
          }
        }
      }
    } else {
      validate15.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate15.errors = vErrors;
  return errors === 0;
}
var validateDownloadListOutcome = validate16;
var schema19 = { "additionalProperties": false, "properties": { "downloadId": { "type": "string" }, "downloadedBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "error": { "type": ["string", "null"] }, "etaSeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "libraryModelId": { "pumasPortablePath": true, "pumasUtf8Max": 4096, "type": ["string", "null"] }, "modelName": { "type": ["string", "null"] }, "modelType": { "type": ["string", "null"] }, "nextRetryDelaySeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "repoId": { "type": ["string", "null"] }, "retryAttempt": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retryLimit": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retrying": { "type": ["boolean", "null"] }, "selectedArtifactId": { "type": ["string", "null"] }, "speed": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "status": { "$ref": "#/definitions/DownloadStatus" }, "totalBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["downloadId", "libraryModelId", "repoId", "selectedArtifactId", "modelName", "modelType", "status", "progress", "downloadedBytes", "totalBytes", "speed", "etaSeconds", "retryAttempt", "retryLimit", "retrying", "nextRetryDelaySeconds", "error"], "type": "object" };
var schema20 = { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" };
function validate17(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.downloadId === void 0 && (missing0 = "downloadId") || data.libraryModelId === void 0 && (missing0 = "libraryModelId") || data.repoId === void 0 && (missing0 = "repoId") || data.selectedArtifactId === void 0 && (missing0 = "selectedArtifactId") || data.modelName === void 0 && (missing0 = "modelName") || data.modelType === void 0 && (missing0 = "modelType") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.downloadedBytes === void 0 && (missing0 = "downloadedBytes") || data.totalBytes === void 0 && (missing0 = "totalBytes") || data.speed === void 0 && (missing0 = "speed") || data.etaSeconds === void 0 && (missing0 = "etaSeconds") || data.retryAttempt === void 0 && (missing0 = "retryAttempt") || data.retryLimit === void 0 && (missing0 = "retryLimit") || data.retrying === void 0 && (missing0 = "retrying") || data.nextRetryDelaySeconds === void 0 && (missing0 = "nextRetryDelaySeconds") || data.error === void 0 && (missing0 = "error")) {
        validate17.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema19.properties, key0)) {
            validate17.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.downloadId !== void 0) {
            const _errs2 = errors;
            if (typeof data.downloadId !== "string") {
              validate17.errors = [{ instancePath: instancePath + "/downloadId", schemaPath: "#/properties/downloadId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.downloadedBytes !== void 0) {
              let data1 = data.downloadedBytes;
              const _errs4 = errors;
              if (!(typeof data1 == "number" && (!(data1 % 1) && !isNaN(data1)) && isFinite(data1)) && data1 !== null) {
                validate17.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/type", keyword: "type", params: { type: schema19.properties.downloadedBytes.type }, message: "must be integer,null" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 9007199254740991 || isNaN(data1)) {
                    validate17.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate17.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                      return false;
                    }
                  }
                }
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.error !== void 0) {
                let data2 = data.error;
                const _errs6 = errors;
                if (typeof data2 !== "string" && data2 !== null) {
                  validate17.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema19.properties.error.type }, message: "must be string,null" }];
                  return false;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.etaSeconds !== void 0) {
                  let data3 = data.etaSeconds;
                  const _errs8 = errors;
                  if (!(typeof data3 == "number" && isFinite(data3)) && data3 !== null) {
                    validate17.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/type", keyword: "type", params: { type: schema19.properties.etaSeconds.type }, message: "must be number,null" }];
                    return false;
                  }
                  if (errors === _errs8) {
                    if (typeof data3 == "number" && isFinite(data3)) {
                      if (data3 > 17976931348623157e292 || isNaN(data3)) {
                        validate17.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                        return false;
                      } else {
                        if (data3 < 0 || isNaN(data3)) {
                          validate17.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                          return false;
                        }
                      }
                    }
                  }
                  var valid0 = _errs8 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.libraryModelId !== void 0) {
                    let data4 = data.libraryModelId;
                    const _errs10 = errors;
                    if (typeof data4 !== "string" && data4 !== null) {
                      validate17.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/type", keyword: "type", params: { type: schema19.properties.libraryModelId.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (errors === _errs10) {
                      if (typeof data4 === "string") {
                        if (data4.length === 0 || data4.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(data4) || Array.from(data4).some((letter) => letter.codePointAt(0) < 32 || letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159) || data4.split("/").some((component) => {
                          const stem = component.split(".")[0].replace(/[a-z]/g, (letter) => letter.toUpperCase());
                          return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g, "x").length > 255 || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
                        })) {
                          validate17.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate17.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                            return false;
                          }
                        }
                      }
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.modelName !== void 0) {
                      let data5 = data.modelName;
                      const _errs12 = errors;
                      if (typeof data5 !== "string" && data5 !== null) {
                        validate17.errors = [{ instancePath: instancePath + "/modelName", schemaPath: "#/properties/modelName/type", keyword: "type", params: { type: schema19.properties.modelName.type }, message: "must be string,null" }];
                        return false;
                      }
                      var valid0 = _errs12 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.modelType !== void 0) {
                        let data6 = data.modelType;
                        const _errs14 = errors;
                        if (typeof data6 !== "string" && data6 !== null) {
                          validate17.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: schema19.properties.modelType.type }, message: "must be string,null" }];
                          return false;
                        }
                        var valid0 = _errs14 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.nextRetryDelaySeconds !== void 0) {
                          let data7 = data.nextRetryDelaySeconds;
                          const _errs16 = errors;
                          if (!(typeof data7 == "number" && isFinite(data7)) && data7 !== null) {
                            validate17.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/type", keyword: "type", params: { type: schema19.properties.nextRetryDelaySeconds.type }, message: "must be number,null" }];
                            return false;
                          }
                          if (errors === _errs16) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 17976931348623157e292 || isNaN(data7)) {
                                validate17.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate17.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                  return false;
                                }
                              }
                            }
                          }
                          var valid0 = _errs16 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.progress !== void 0) {
                            let data8 = data.progress;
                            const _errs18 = errors;
                            if (!(typeof data8 == "number" && isFinite(data8)) && data8 !== null) {
                              validate17.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema19.properties.progress.type }, message: "must be number,null" }];
                              return false;
                            }
                            if (errors === _errs18) {
                              if (typeof data8 == "number" && isFinite(data8)) {
                                if (data8 > 1 || isNaN(data8)) {
                                  validate17.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                  return false;
                                } else {
                                  if (data8 < 0 || isNaN(data8)) {
                                    validate17.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                    return false;
                                  }
                                }
                              }
                            }
                            var valid0 = _errs18 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.repoId !== void 0) {
                              let data9 = data.repoId;
                              const _errs20 = errors;
                              if (typeof data9 !== "string" && data9 !== null) {
                                validate17.errors = [{ instancePath: instancePath + "/repoId", schemaPath: "#/properties/repoId/type", keyword: "type", params: { type: schema19.properties.repoId.type }, message: "must be string,null" }];
                                return false;
                              }
                              var valid0 = _errs20 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.retryAttempt !== void 0) {
                                let data10 = data.retryAttempt;
                                const _errs22 = errors;
                                if (!(typeof data10 == "number" && (!(data10 % 1) && !isNaN(data10)) && isFinite(data10)) && data10 !== null) {
                                  validate17.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/type", keyword: "type", params: { type: schema19.properties.retryAttempt.type }, message: "must be integer,null" }];
                                  return false;
                                }
                                if (errors === _errs22) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 4294967295 || isNaN(data10)) {
                                      validate17.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate17.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                        return false;
                                      }
                                    }
                                  }
                                }
                                var valid0 = _errs22 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.retryLimit !== void 0) {
                                  let data11 = data.retryLimit;
                                  const _errs24 = errors;
                                  if (!(typeof data11 == "number" && (!(data11 % 1) && !isNaN(data11)) && isFinite(data11)) && data11 !== null) {
                                    validate17.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/type", keyword: "type", params: { type: schema19.properties.retryLimit.type }, message: "must be integer,null" }];
                                    return false;
                                  }
                                  if (errors === _errs24) {
                                    if (typeof data11 == "number" && isFinite(data11)) {
                                      if (data11 > 4294967295 || isNaN(data11)) {
                                        validate17.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                        return false;
                                      } else {
                                        if (data11 < 0 || isNaN(data11)) {
                                          validate17.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                          return false;
                                        }
                                      }
                                    }
                                  }
                                  var valid0 = _errs24 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.retrying !== void 0) {
                                    let data12 = data.retrying;
                                    const _errs26 = errors;
                                    if (typeof data12 !== "boolean" && data12 !== null) {
                                      validate17.errors = [{ instancePath: instancePath + "/retrying", schemaPath: "#/properties/retrying/type", keyword: "type", params: { type: schema19.properties.retrying.type }, message: "must be boolean,null" }];
                                      return false;
                                    }
                                    var valid0 = _errs26 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.selectedArtifactId !== void 0) {
                                      let data13 = data.selectedArtifactId;
                                      const _errs28 = errors;
                                      if (typeof data13 !== "string" && data13 !== null) {
                                        validate17.errors = [{ instancePath: instancePath + "/selectedArtifactId", schemaPath: "#/properties/selectedArtifactId/type", keyword: "type", params: { type: schema19.properties.selectedArtifactId.type }, message: "must be string,null" }];
                                        return false;
                                      }
                                      var valid0 = _errs28 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                    if (valid0) {
                                      if (data.speed !== void 0) {
                                        let data14 = data.speed;
                                        const _errs30 = errors;
                                        if (!(typeof data14 == "number" && isFinite(data14)) && data14 !== null) {
                                          validate17.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/type", keyword: "type", params: { type: schema19.properties.speed.type }, message: "must be number,null" }];
                                          return false;
                                        }
                                        if (errors === _errs30) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 17976931348623157e292 || isNaN(data14)) {
                                              validate17.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate17.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                return false;
                                              }
                                            }
                                          }
                                        }
                                        var valid0 = _errs30 === errors;
                                      } else {
                                        var valid0 = true;
                                      }
                                      if (valid0) {
                                        if (data.status !== void 0) {
                                          let data15 = data.status;
                                          const _errs32 = errors;
                                          if (typeof data15 !== "string") {
                                            validate17.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                            return false;
                                          }
                                          if (!(data15 === "queued" || data15 === "downloading" || data15 === "pausing" || data15 === "paused" || data15 === "cancelling" || data15 === "completed" || data15 === "cancelled" || data15 === "error")) {
                                            validate17.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/enum", keyword: "enum", params: { allowedValues: schema20.enum }, message: "must be equal to one of the allowed values" }];
                                            return false;
                                          }
                                          var valid0 = _errs32 === errors;
                                        } else {
                                          var valid0 = true;
                                        }
                                        if (valid0) {
                                          if (data.totalBytes !== void 0) {
                                            let data16 = data.totalBytes;
                                            const _errs35 = errors;
                                            if (!(typeof data16 == "number" && (!(data16 % 1) && !isNaN(data16)) && isFinite(data16)) && data16 !== null) {
                                              validate17.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/type", keyword: "type", params: { type: schema19.properties.totalBytes.type }, message: "must be integer,null" }];
                                              return false;
                                            }
                                            if (errors === _errs35) {
                                              if (typeof data16 == "number" && isFinite(data16)) {
                                                if (data16 > 9007199254740991 || isNaN(data16)) {
                                                  validate17.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                                  return false;
                                                } else {
                                                  if (data16 < 0 || isNaN(data16)) {
                                                    validate17.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                    return false;
                                                  }
                                                }
                                              }
                                            }
                                            var valid0 = _errs35 === errors;
                                          } else {
                                            var valid0 = true;
                                          }
                                        }
                                      }
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate17.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate17.errors = vErrors;
  return errors === 0;
}
function validate16(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.downloads === void 0 && (missing0 = "downloads")) {
        validate16.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "downloads" || key0 === "success")) {
            validate16.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.downloads !== void 0) {
            let data0 = data.downloads;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (!validate17(data0[i0], { instancePath: instancePath + "/downloads/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate17.errors : vErrors.concat(validate17.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate16.errors = [{ instancePath: instancePath + "/downloads", schemaPath: "#/properties/downloads/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.success !== void 0) {
              let data2 = data.success;
              const _errs5 = errors;
              if (typeof data2 !== "boolean") {
                validate16.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate16.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                return false;
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate16.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate16.errors = vErrors;
  return errors === 0;
}
var validateDownloadMutationOutcome = validate19;
function validate19(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success")) {
        validate19.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "error" || key0 === "success")) {
            validate19.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.error !== void 0) {
            const _errs2 = errors;
            if (typeof data.error !== "string") {
              validate19.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.success !== void 0) {
              const _errs4 = errors;
              if (typeof data.success !== "boolean") {
                validate19.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.success === true && data.error !== void 0 || data.success === false && typeof data.error !== "string") {
                validate19.errors = [{ instancePath, schemaPath: "#/pumasMutation", keyword: "pumasMutation", params: {}, message: 'must pass "pumasMutation" keyword validation' }];
                return false;
              }
            }
          }
        }
      }
    } else {
      validate19.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate19.errors = vErrors;
  return errors === 0;
}
var validateDownloadStartedOutcome = validate20;
var schema23 = { "additionalProperties": false, "properties": { "artifactId": { "type": ["string", "null"] }, "download_id": { "type": "string" }, "selectedArtifactId": { "type": ["string", "null"] }, "success": { "const": true, "type": "boolean" } }, "pumasStarted": true, "required": ["success", "download_id", "selectedArtifactId", "artifactId"], "type": "object" };
function validate20(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  const _errs2 = errors;
  if (errors === _errs2) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.download_id === void 0 && (missing0 = "download_id") || data.selectedArtifactId === void 0 && (missing0 = "selectedArtifactId") || data.artifactId === void 0 && (missing0 = "artifactId")) {
        const err0 = { instancePath, schemaPath: "#/definitions/DownloadStartedSuccess/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs4 = errors;
        for (const key0 in data) {
          if (!(key0 === "artifactId" || key0 === "download_id" || key0 === "selectedArtifactId" || key0 === "success")) {
            const err1 = { instancePath, schemaPath: "#/definitions/DownloadStartedSuccess/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err1];
            } else {
              vErrors.push(err1);
            }
            errors++;
            break;
          }
        }
        if (_errs4 === errors) {
          if (data.artifactId !== void 0) {
            let data0 = data.artifactId;
            const _errs5 = errors;
            if (typeof data0 !== "string" && data0 !== null) {
              const err2 = { instancePath: instancePath + "/artifactId", schemaPath: "#/definitions/DownloadStartedSuccess/properties/artifactId/type", keyword: "type", params: { type: schema23.properties.artifactId.type }, message: "must be string,null" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            var valid2 = _errs5 === errors;
          } else {
            var valid2 = true;
          }
          if (valid2) {
            if (data.download_id !== void 0) {
              const _errs7 = errors;
              if (typeof data.download_id !== "string") {
                const err3 = { instancePath: instancePath + "/download_id", schemaPath: "#/definitions/DownloadStartedSuccess/properties/download_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err3];
                } else {
                  vErrors.push(err3);
                }
                errors++;
              }
              var valid2 = _errs7 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.selectedArtifactId !== void 0) {
                let data2 = data.selectedArtifactId;
                const _errs9 = errors;
                if (typeof data2 !== "string" && data2 !== null) {
                  const err4 = { instancePath: instancePath + "/selectedArtifactId", schemaPath: "#/definitions/DownloadStartedSuccess/properties/selectedArtifactId/type", keyword: "type", params: { type: schema23.properties.selectedArtifactId.type }, message: "must be string,null" };
                  if (vErrors === null) {
                    vErrors = [err4];
                  } else {
                    vErrors.push(err4);
                  }
                  errors++;
                }
                var valid2 = _errs9 === errors;
              } else {
                var valid2 = true;
              }
              if (valid2) {
                if (data.success !== void 0) {
                  let data3 = data.success;
                  const _errs11 = errors;
                  if (typeof data3 !== "boolean") {
                    const err5 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/DownloadStartedSuccess/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                    if (vErrors === null) {
                      vErrors = [err5];
                    } else {
                      vErrors.push(err5);
                    }
                    errors++;
                  }
                  if (true !== data3) {
                    const err6 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/DownloadStartedSuccess/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err6];
                    } else {
                      vErrors.push(err6);
                    }
                    errors++;
                  }
                  var valid2 = _errs11 === errors;
                } else {
                  var valid2 = true;
                }
                if (valid2) {
                  if (data.selectedArtifactId !== data.artifactId) {
                    const err7 = { instancePath, schemaPath: "#/definitions/DownloadStartedSuccess/pumasStarted", keyword: "pumasStarted", params: {}, message: 'must pass "pumasStarted" keyword validation' };
                    if (vErrors === null) {
                      vErrors = [err7];
                    } else {
                      vErrors.push(err7);
                    }
                    errors++;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      const err8 = { instancePath, schemaPath: "#/definitions/DownloadStartedSuccess/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs13 = errors;
    const _errs14 = errors;
    if (errors === _errs14) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.success === void 0 && (missing1 = "success") || data.error === void 0 && (missing1 = "error")) {
          const err9 = { instancePath, schemaPath: "#/definitions/DownloadStartedFailure/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        } else {
          const _errs16 = errors;
          for (const key1 in data) {
            if (!(key1 === "error" || key1 === "success")) {
              const err10 = { instancePath, schemaPath: "#/definitions/DownloadStartedFailure/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err10];
              } else {
                vErrors.push(err10);
              }
              errors++;
              break;
            }
          }
          if (_errs16 === errors) {
            if (data.error !== void 0) {
              const _errs17 = errors;
              if (typeof data.error !== "string") {
                const err11 = { instancePath: instancePath + "/error", schemaPath: "#/definitions/DownloadStartedFailure/properties/error/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err11];
                } else {
                  vErrors.push(err11);
                }
                errors++;
              }
              var valid4 = _errs17 === errors;
            } else {
              var valid4 = true;
            }
            if (valid4) {
              if (data.success !== void 0) {
                let data5 = data.success;
                const _errs19 = errors;
                if (typeof data5 !== "boolean") {
                  const err12 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/DownloadStartedFailure/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                  if (vErrors === null) {
                    vErrors = [err12];
                  } else {
                    vErrors.push(err12);
                  }
                  errors++;
                }
                if (false !== data5) {
                  const err13 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/DownloadStartedFailure/properties/success/const", keyword: "const", params: { allowedValue: false }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err13];
                  } else {
                    vErrors.push(err13);
                  }
                  errors++;
                }
                var valid4 = _errs19 === errors;
              } else {
                var valid4 = true;
              }
            }
          }
        }
      } else {
        const err14 = { instancePath, schemaPath: "#/definitions/DownloadStartedFailure/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err14];
        } else {
          vErrors.push(err14);
        }
        errors++;
      }
    }
    var _valid0 = _errs13 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err15 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err15];
    } else {
      vErrors.push(err15);
    }
    errors++;
    validate20.errors = vErrors;
    return false;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate20.errors = vErrors;
  return errors === 0;
}
var validateDownloadStatusOutcome = validate21;
var schema26 = { "additionalProperties": false, "properties": { "downloadId": { "type": "string" }, "downloadedBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "error": { "type": ["string", "null"] }, "etaSeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "libraryModelId": { "pumasPortablePath": true, "pumasUtf8Max": 4096, "type": ["string", "null"] }, "modelName": { "type": ["string", "null"] }, "modelType": { "type": ["string", "null"] }, "nextRetryDelaySeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "repoId": { "type": ["string", "null"] }, "retryAttempt": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retryLimit": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retrying": { "type": ["boolean", "null"] }, "selectedArtifactId": { "type": ["string", "null"] }, "speed": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "status": { "$ref": "#/definitions/DownloadStatus" }, "success": { "const": true, "type": "boolean" }, "totalBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["success", "downloadId", "libraryModelId", "repoId", "selectedArtifactId", "modelName", "modelType", "status", "progress", "downloadedBytes", "totalBytes", "speed", "etaSeconds", "retryAttempt", "retryLimit", "retrying", "nextRetryDelaySeconds", "error"], "type": "object" };
var schema27 = { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" };
function validate22(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.downloadId === void 0 && (missing0 = "downloadId") || data.libraryModelId === void 0 && (missing0 = "libraryModelId") || data.repoId === void 0 && (missing0 = "repoId") || data.selectedArtifactId === void 0 && (missing0 = "selectedArtifactId") || data.modelName === void 0 && (missing0 = "modelName") || data.modelType === void 0 && (missing0 = "modelType") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.downloadedBytes === void 0 && (missing0 = "downloadedBytes") || data.totalBytes === void 0 && (missing0 = "totalBytes") || data.speed === void 0 && (missing0 = "speed") || data.etaSeconds === void 0 && (missing0 = "etaSeconds") || data.retryAttempt === void 0 && (missing0 = "retryAttempt") || data.retryLimit === void 0 && (missing0 = "retryLimit") || data.retrying === void 0 && (missing0 = "retrying") || data.nextRetryDelaySeconds === void 0 && (missing0 = "nextRetryDelaySeconds") || data.error === void 0 && (missing0 = "error")) {
        validate22.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema26.properties, key0)) {
            validate22.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.downloadId !== void 0) {
            const _errs2 = errors;
            if (typeof data.downloadId !== "string") {
              validate22.errors = [{ instancePath: instancePath + "/downloadId", schemaPath: "#/properties/downloadId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.downloadedBytes !== void 0) {
              let data1 = data.downloadedBytes;
              const _errs4 = errors;
              if (!(typeof data1 == "number" && (!(data1 % 1) && !isNaN(data1)) && isFinite(data1)) && data1 !== null) {
                validate22.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/type", keyword: "type", params: { type: schema26.properties.downloadedBytes.type }, message: "must be integer,null" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 9007199254740991 || isNaN(data1)) {
                    validate22.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate22.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                      return false;
                    }
                  }
                }
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.error !== void 0) {
                let data2 = data.error;
                const _errs6 = errors;
                if (typeof data2 !== "string" && data2 !== null) {
                  validate22.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema26.properties.error.type }, message: "must be string,null" }];
                  return false;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.etaSeconds !== void 0) {
                  let data3 = data.etaSeconds;
                  const _errs8 = errors;
                  if (!(typeof data3 == "number" && isFinite(data3)) && data3 !== null) {
                    validate22.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/type", keyword: "type", params: { type: schema26.properties.etaSeconds.type }, message: "must be number,null" }];
                    return false;
                  }
                  if (errors === _errs8) {
                    if (typeof data3 == "number" && isFinite(data3)) {
                      if (data3 > 17976931348623157e292 || isNaN(data3)) {
                        validate22.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                        return false;
                      } else {
                        if (data3 < 0 || isNaN(data3)) {
                          validate22.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                          return false;
                        }
                      }
                    }
                  }
                  var valid0 = _errs8 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.libraryModelId !== void 0) {
                    let data4 = data.libraryModelId;
                    const _errs10 = errors;
                    if (typeof data4 !== "string" && data4 !== null) {
                      validate22.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/type", keyword: "type", params: { type: schema26.properties.libraryModelId.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (errors === _errs10) {
                      if (typeof data4 === "string") {
                        if (data4.length === 0 || data4.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(data4) || Array.from(data4).some((letter) => letter.codePointAt(0) < 32 || letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159) || data4.split("/").some((component) => {
                          const stem = component.split(".")[0].replace(/[a-z]/g, (letter) => letter.toUpperCase());
                          return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g, "x").length > 255 || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
                        })) {
                          validate22.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate22.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                            return false;
                          }
                        }
                      }
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.modelName !== void 0) {
                      let data5 = data.modelName;
                      const _errs12 = errors;
                      if (typeof data5 !== "string" && data5 !== null) {
                        validate22.errors = [{ instancePath: instancePath + "/modelName", schemaPath: "#/properties/modelName/type", keyword: "type", params: { type: schema26.properties.modelName.type }, message: "must be string,null" }];
                        return false;
                      }
                      var valid0 = _errs12 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.modelType !== void 0) {
                        let data6 = data.modelType;
                        const _errs14 = errors;
                        if (typeof data6 !== "string" && data6 !== null) {
                          validate22.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: schema26.properties.modelType.type }, message: "must be string,null" }];
                          return false;
                        }
                        var valid0 = _errs14 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.nextRetryDelaySeconds !== void 0) {
                          let data7 = data.nextRetryDelaySeconds;
                          const _errs16 = errors;
                          if (!(typeof data7 == "number" && isFinite(data7)) && data7 !== null) {
                            validate22.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/type", keyword: "type", params: { type: schema26.properties.nextRetryDelaySeconds.type }, message: "must be number,null" }];
                            return false;
                          }
                          if (errors === _errs16) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 17976931348623157e292 || isNaN(data7)) {
                                validate22.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate22.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                  return false;
                                }
                              }
                            }
                          }
                          var valid0 = _errs16 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.progress !== void 0) {
                            let data8 = data.progress;
                            const _errs18 = errors;
                            if (!(typeof data8 == "number" && isFinite(data8)) && data8 !== null) {
                              validate22.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema26.properties.progress.type }, message: "must be number,null" }];
                              return false;
                            }
                            if (errors === _errs18) {
                              if (typeof data8 == "number" && isFinite(data8)) {
                                if (data8 > 1 || isNaN(data8)) {
                                  validate22.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                  return false;
                                } else {
                                  if (data8 < 0 || isNaN(data8)) {
                                    validate22.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                    return false;
                                  }
                                }
                              }
                            }
                            var valid0 = _errs18 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.repoId !== void 0) {
                              let data9 = data.repoId;
                              const _errs20 = errors;
                              if (typeof data9 !== "string" && data9 !== null) {
                                validate22.errors = [{ instancePath: instancePath + "/repoId", schemaPath: "#/properties/repoId/type", keyword: "type", params: { type: schema26.properties.repoId.type }, message: "must be string,null" }];
                                return false;
                              }
                              var valid0 = _errs20 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.retryAttempt !== void 0) {
                                let data10 = data.retryAttempt;
                                const _errs22 = errors;
                                if (!(typeof data10 == "number" && (!(data10 % 1) && !isNaN(data10)) && isFinite(data10)) && data10 !== null) {
                                  validate22.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/type", keyword: "type", params: { type: schema26.properties.retryAttempt.type }, message: "must be integer,null" }];
                                  return false;
                                }
                                if (errors === _errs22) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 4294967295 || isNaN(data10)) {
                                      validate22.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate22.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                        return false;
                                      }
                                    }
                                  }
                                }
                                var valid0 = _errs22 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.retryLimit !== void 0) {
                                  let data11 = data.retryLimit;
                                  const _errs24 = errors;
                                  if (!(typeof data11 == "number" && (!(data11 % 1) && !isNaN(data11)) && isFinite(data11)) && data11 !== null) {
                                    validate22.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/type", keyword: "type", params: { type: schema26.properties.retryLimit.type }, message: "must be integer,null" }];
                                    return false;
                                  }
                                  if (errors === _errs24) {
                                    if (typeof data11 == "number" && isFinite(data11)) {
                                      if (data11 > 4294967295 || isNaN(data11)) {
                                        validate22.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                        return false;
                                      } else {
                                        if (data11 < 0 || isNaN(data11)) {
                                          validate22.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                          return false;
                                        }
                                      }
                                    }
                                  }
                                  var valid0 = _errs24 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.retrying !== void 0) {
                                    let data12 = data.retrying;
                                    const _errs26 = errors;
                                    if (typeof data12 !== "boolean" && data12 !== null) {
                                      validate22.errors = [{ instancePath: instancePath + "/retrying", schemaPath: "#/properties/retrying/type", keyword: "type", params: { type: schema26.properties.retrying.type }, message: "must be boolean,null" }];
                                      return false;
                                    }
                                    var valid0 = _errs26 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.selectedArtifactId !== void 0) {
                                      let data13 = data.selectedArtifactId;
                                      const _errs28 = errors;
                                      if (typeof data13 !== "string" && data13 !== null) {
                                        validate22.errors = [{ instancePath: instancePath + "/selectedArtifactId", schemaPath: "#/properties/selectedArtifactId/type", keyword: "type", params: { type: schema26.properties.selectedArtifactId.type }, message: "must be string,null" }];
                                        return false;
                                      }
                                      var valid0 = _errs28 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                    if (valid0) {
                                      if (data.speed !== void 0) {
                                        let data14 = data.speed;
                                        const _errs30 = errors;
                                        if (!(typeof data14 == "number" && isFinite(data14)) && data14 !== null) {
                                          validate22.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/type", keyword: "type", params: { type: schema26.properties.speed.type }, message: "must be number,null" }];
                                          return false;
                                        }
                                        if (errors === _errs30) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 17976931348623157e292 || isNaN(data14)) {
                                              validate22.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate22.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                return false;
                                              }
                                            }
                                          }
                                        }
                                        var valid0 = _errs30 === errors;
                                      } else {
                                        var valid0 = true;
                                      }
                                      if (valid0) {
                                        if (data.status !== void 0) {
                                          let data15 = data.status;
                                          const _errs32 = errors;
                                          if (typeof data15 !== "string") {
                                            validate22.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                            return false;
                                          }
                                          if (!(data15 === "queued" || data15 === "downloading" || data15 === "pausing" || data15 === "paused" || data15 === "cancelling" || data15 === "completed" || data15 === "cancelled" || data15 === "error")) {
                                            validate22.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/enum", keyword: "enum", params: { allowedValues: schema27.enum }, message: "must be equal to one of the allowed values" }];
                                            return false;
                                          }
                                          var valid0 = _errs32 === errors;
                                        } else {
                                          var valid0 = true;
                                        }
                                        if (valid0) {
                                          if (data.success !== void 0) {
                                            let data16 = data.success;
                                            const _errs35 = errors;
                                            if (typeof data16 !== "boolean") {
                                              validate22.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                              return false;
                                            }
                                            if (true !== data16) {
                                              validate22.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                                              return false;
                                            }
                                            var valid0 = _errs35 === errors;
                                          } else {
                                            var valid0 = true;
                                          }
                                          if (valid0) {
                                            if (data.totalBytes !== void 0) {
                                              let data17 = data.totalBytes;
                                              const _errs37 = errors;
                                              if (!(typeof data17 == "number" && (!(data17 % 1) && !isNaN(data17)) && isFinite(data17)) && data17 !== null) {
                                                validate22.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/type", keyword: "type", params: { type: schema26.properties.totalBytes.type }, message: "must be integer,null" }];
                                                return false;
                                              }
                                              if (errors === _errs37) {
                                                if (typeof data17 == "number" && isFinite(data17)) {
                                                  if (data17 > 9007199254740991 || isNaN(data17)) {
                                                    validate22.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                                    return false;
                                                  } else {
                                                    if (data17 < 0 || isNaN(data17)) {
                                                      validate22.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                      return false;
                                                    }
                                                  }
                                                }
                                              }
                                              var valid0 = _errs37 === errors;
                                            } else {
                                              var valid0 = true;
                                            }
                                          }
                                        }
                                      }
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate22.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate22.errors = vErrors;
  return errors === 0;
}
function validate21(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (!validate22(data, { instancePath, parentData, parentDataProperty, rootData })) {
    vErrors = vErrors === null ? validate22.errors : vErrors.concat(validate22.errors);
    errors = vErrors.length;
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing0;
        if (data.success === void 0 && (missing0 = "success") || data.error === void 0 && (missing0 = "error")) {
          const err0 = { instancePath, schemaPath: "#/definitions/DownloadStatusMissingOutcome/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
          if (vErrors === null) {
            vErrors = [err0];
          } else {
            vErrors.push(err0);
          }
          errors++;
        } else {
          const _errs5 = errors;
          for (const key0 in data) {
            if (!(key0 === "error" || key0 === "success")) {
              const err1 = { instancePath, schemaPath: "#/definitions/DownloadStatusMissingOutcome/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err1];
              } else {
                vErrors.push(err1);
              }
              errors++;
              break;
            }
          }
          if (_errs5 === errors) {
            if (data.error !== void 0) {
              const _errs6 = errors;
              if (typeof data.error !== "string") {
                const err2 = { instancePath: instancePath + "/error", schemaPath: "#/definitions/DownloadStatusMissingOutcome/properties/error/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err2];
                } else {
                  vErrors.push(err2);
                }
                errors++;
              }
              var valid2 = _errs6 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.success !== void 0) {
                let data1 = data.success;
                const _errs8 = errors;
                if (typeof data1 !== "boolean") {
                  const err3 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/DownloadStatusMissingOutcome/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                  if (vErrors === null) {
                    vErrors = [err3];
                  } else {
                    vErrors.push(err3);
                  }
                  errors++;
                }
                if (false !== data1) {
                  const err4 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/DownloadStatusMissingOutcome/properties/success/const", keyword: "const", params: { allowedValue: false }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err4];
                  } else {
                    vErrors.push(err4);
                  }
                  errors++;
                }
                var valid2 = _errs8 === errors;
              } else {
                var valid2 = true;
              }
            }
          }
        }
      } else {
        const err5 = { instancePath, schemaPath: "#/definitions/DownloadStatusMissingOutcome/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    var _valid0 = _errs2 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
    validate21.errors = vErrors;
    return false;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate21.errors = vErrors;
  return errors === 0;
}
var validateLinkHealthOutcome = validate24;
var schema30 = { "additionalProperties": false, "description": "Link health response.\n\nNote: Not FFI-compatible due to `usize` fields. Use wrapper types in pumas-uniffi.", "properties": { "broken_links": { "items": { "type": "string" }, "type": "array" }, "error": { "type": "null" }, "errors": { "items": { "type": "string" }, "type": "array" }, "healthy_links": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" }, "orphaned_links": { "items": { "type": "string" }, "type": "array" }, "status": { "enum": ["healthy", "degraded"], "type": "string" }, "success": { "const": true, "type": "boolean" }, "total_links": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" }, "warnings": { "items": { "type": "string" }, "type": "array" } }, "pumasLinkHealth": true, "required": ["success", "status", "total_links", "healthy_links", "broken_links", "orphaned_links", "warnings", "errors"], "type": "object" };
function validate24(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.status === void 0 && (missing0 = "status") || data.total_links === void 0 && (missing0 = "total_links") || data.healthy_links === void 0 && (missing0 = "healthy_links") || data.broken_links === void 0 && (missing0 = "broken_links") || data.orphaned_links === void 0 && (missing0 = "orphaned_links") || data.warnings === void 0 && (missing0 = "warnings") || data.errors === void 0 && (missing0 = "errors")) {
        validate24.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (!func2.call(schema30.properties, key0)) {
            validate24.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs3 === errors) {
          if (data.broken_links !== void 0) {
            let data0 = data.broken_links;
            const _errs4 = errors;
            if (errors === _errs4) {
              if (Array.isArray(data0)) {
                var valid3 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs6 = errors;
                  if (typeof data0[i0] !== "string") {
                    validate24.errors = [{ instancePath: instancePath + "/broken_links/" + i0, schemaPath: "#/definitions/LinkHealthResponse/properties/broken_links/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                  var valid3 = _errs6 === errors;
                  if (!valid3) {
                    break;
                  }
                }
              } else {
                validate24.errors = [{ instancePath: instancePath + "/broken_links", schemaPath: "#/definitions/LinkHealthResponse/properties/broken_links/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                return false;
              }
            }
            var valid2 = _errs4 === errors;
          } else {
            var valid2 = true;
          }
          if (valid2) {
            if (data.error !== void 0) {
              const _errs8 = errors;
              if (data.error !== null) {
                validate24.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/definitions/LinkHealthResponse/properties/error/type", keyword: "type", params: { type: "null" }, message: "must be null" }];
                return false;
              }
              var valid2 = _errs8 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.errors !== void 0) {
                let data3 = data.errors;
                const _errs10 = errors;
                if (errors === _errs10) {
                  if (Array.isArray(data3)) {
                    var valid4 = true;
                    const len1 = data3.length;
                    for (let i1 = 0; i1 < len1; i1++) {
                      const _errs12 = errors;
                      if (typeof data3[i1] !== "string") {
                        validate24.errors = [{ instancePath: instancePath + "/errors/" + i1, schemaPath: "#/definitions/LinkHealthResponse/properties/errors/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                      var valid4 = _errs12 === errors;
                      if (!valid4) {
                        break;
                      }
                    }
                  } else {
                    validate24.errors = [{ instancePath: instancePath + "/errors", schemaPath: "#/definitions/LinkHealthResponse/properties/errors/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                    return false;
                  }
                }
                var valid2 = _errs10 === errors;
              } else {
                var valid2 = true;
              }
              if (valid2) {
                if (data.healthy_links !== void 0) {
                  let data5 = data.healthy_links;
                  const _errs14 = errors;
                  if (!(typeof data5 == "number" && (!(data5 % 1) && !isNaN(data5)) && isFinite(data5))) {
                    validate24.errors = [{ instancePath: instancePath + "/healthy_links", schemaPath: "#/definitions/LinkHealthResponse/properties/healthy_links/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                    return false;
                  }
                  if (errors === _errs14) {
                    if (typeof data5 == "number" && isFinite(data5)) {
                      if (data5 > 9007199254740991 || isNaN(data5)) {
                        validate24.errors = [{ instancePath: instancePath + "/healthy_links", schemaPath: "#/definitions/LinkHealthResponse/properties/healthy_links/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                        return false;
                      } else {
                        if (data5 < 0 || isNaN(data5)) {
                          validate24.errors = [{ instancePath: instancePath + "/healthy_links", schemaPath: "#/definitions/LinkHealthResponse/properties/healthy_links/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                          return false;
                        }
                      }
                    }
                  }
                  var valid2 = _errs14 === errors;
                } else {
                  var valid2 = true;
                }
                if (valid2) {
                  if (data.orphaned_links !== void 0) {
                    let data6 = data.orphaned_links;
                    const _errs16 = errors;
                    if (errors === _errs16) {
                      if (Array.isArray(data6)) {
                        var valid5 = true;
                        const len2 = data6.length;
                        for (let i2 = 0; i2 < len2; i2++) {
                          const _errs18 = errors;
                          if (typeof data6[i2] !== "string") {
                            validate24.errors = [{ instancePath: instancePath + "/orphaned_links/" + i2, schemaPath: "#/definitions/LinkHealthResponse/properties/orphaned_links/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                            return false;
                          }
                          var valid5 = _errs18 === errors;
                          if (!valid5) {
                            break;
                          }
                        }
                      } else {
                        validate24.errors = [{ instancePath: instancePath + "/orphaned_links", schemaPath: "#/definitions/LinkHealthResponse/properties/orphaned_links/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                        return false;
                      }
                    }
                    var valid2 = _errs16 === errors;
                  } else {
                    var valid2 = true;
                  }
                  if (valid2) {
                    if (data.status !== void 0) {
                      let data8 = data.status;
                      const _errs20 = errors;
                      if (typeof data8 !== "string") {
                        validate24.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/LinkHealthResponse/properties/status/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                      if (!(data8 === "healthy" || data8 === "degraded")) {
                        validate24.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/LinkHealthResponse/properties/status/enum", keyword: "enum", params: { allowedValues: schema30.properties.status.enum }, message: "must be equal to one of the allowed values" }];
                        return false;
                      }
                      var valid2 = _errs20 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data.success !== void 0) {
                        let data9 = data.success;
                        const _errs22 = errors;
                        if (typeof data9 !== "boolean") {
                          validate24.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/definitions/LinkHealthResponse/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                          return false;
                        }
                        if (true !== data9) {
                          validate24.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/definitions/LinkHealthResponse/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                          return false;
                        }
                        var valid2 = _errs22 === errors;
                      } else {
                        var valid2 = true;
                      }
                      if (valid2) {
                        if (data.total_links !== void 0) {
                          let data10 = data.total_links;
                          const _errs24 = errors;
                          if (!(typeof data10 == "number" && (!(data10 % 1) && !isNaN(data10)) && isFinite(data10))) {
                            validate24.errors = [{ instancePath: instancePath + "/total_links", schemaPath: "#/definitions/LinkHealthResponse/properties/total_links/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                            return false;
                          }
                          if (errors === _errs24) {
                            if (typeof data10 == "number" && isFinite(data10)) {
                              if (data10 > 9007199254740991 || isNaN(data10)) {
                                validate24.errors = [{ instancePath: instancePath + "/total_links", schemaPath: "#/definitions/LinkHealthResponse/properties/total_links/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                return false;
                              } else {
                                if (data10 < 0 || isNaN(data10)) {
                                  validate24.errors = [{ instancePath: instancePath + "/total_links", schemaPath: "#/definitions/LinkHealthResponse/properties/total_links/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                  return false;
                                }
                              }
                            }
                          }
                          var valid2 = _errs24 === errors;
                        } else {
                          var valid2 = true;
                        }
                        if (valid2) {
                          if (data.warnings !== void 0) {
                            let data11 = data.warnings;
                            const _errs26 = errors;
                            if (errors === _errs26) {
                              if (Array.isArray(data11)) {
                                var valid6 = true;
                                const len3 = data11.length;
                                for (let i3 = 0; i3 < len3; i3++) {
                                  const _errs28 = errors;
                                  if (typeof data11[i3] !== "string") {
                                    validate24.errors = [{ instancePath: instancePath + "/warnings/" + i3, schemaPath: "#/definitions/LinkHealthResponse/properties/warnings/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                    return false;
                                  }
                                  var valid6 = _errs28 === errors;
                                  if (!valid6) {
                                    break;
                                  }
                                }
                              } else {
                                validate24.errors = [{ instancePath: instancePath + "/warnings", schemaPath: "#/definitions/LinkHealthResponse/properties/warnings/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                return false;
                              }
                            }
                            var valid2 = _errs26 === errors;
                          } else {
                            var valid2 = true;
                          }
                          if (valid2) {
                            if (!Array.isArray(data.broken_links) || data.healthy_links + data.broken_links.length !== data.total_links || data.status === "healthy" !== (data.broken_links.length === 0)) {
                              validate24.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/pumasLinkHealth", keyword: "pumasLinkHealth", params: {}, message: 'must pass "pumasLinkHealth" keyword validation' }];
                              return false;
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate24.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (!Array.isArray(data.broken_links) || data.healthy_links + data.broken_links.length !== data.total_links || data.status === "healthy" !== (data.broken_links.length === 0)) {
        validate24.errors = [{ instancePath, schemaPath: "#/pumasLinkHealth", keyword: "pumasLinkHealth", params: {}, message: 'must pass "pumasLinkHealth" keyword validation' }];
        return false;
      }
    }
  }
  validate24.errors = vErrors;
  return errors === 0;
}
var validateModelIndexRefreshOutcome = validate25;
function validate25(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.indexed_count === void 0 && (missing0 = "indexed_count")) {
        validate25.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "indexed_count" || key0 === "success")) {
            validate25.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.indexed_count !== void 0) {
            let data0 = data.indexed_count;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0))) {
              validate25.errors = [{ instancePath: instancePath + "/indexed_count", schemaPath: "#/properties/indexed_count/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 4294967295 || isNaN(data0)) {
                  validate25.errors = [{ instancePath: instancePath + "/indexed_count", schemaPath: "#/properties/indexed_count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                  return false;
                } else {
                  if (data0 < 0 || isNaN(data0)) {
                    validate25.errors = [{ instancePath: instancePath + "/indexed_count", schemaPath: "#/properties/indexed_count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                    return false;
                  }
                }
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.success !== void 0) {
              let data1 = data.success;
              const _errs4 = errors;
              if (typeof data1 !== "boolean") {
                validate25.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate25.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate25.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate25.errors = vErrors;
  return errors === 0;
}
var validateModelsOutcome = validate26;
var schema33 = { "additionalProperties": false, "properties": { "artifact": { "$ref": "#/definitions/CatalogArtifactState" }, "dependencyCount": { "maximum": 512, "minimum": 0, "type": "integer" }, "displayDate": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "displayName": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "format": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "id": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "integrity": { "$ref": "#/definitions/CatalogIntegrityState" }, "modelDir": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "modelType": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "quantization": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "relatedAvailable": { "type": "boolean" }, "sizeBytes": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" } }, "pumasCatalogRow": true, "required": ["id", "modelDir", "displayName", "modelType", "dependencyCount", "relatedAvailable", "artifact", "integrity"], "type": "object" };
var schema35 = { "enum": ["part_file_present", "expected_files_missing"], "type": "string" };
var pattern14 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern15 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern16 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate28(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.state === void 0 && (missing0 = "state")) {
        const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (!(key0 === "state")) {
            const err1 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err1];
            } else {
              vErrors.push(err1);
            }
            errors++;
            break;
          }
        }
        if (_errs3 === errors) {
          if (data.state !== void 0) {
            let data0 = data.state;
            if (typeof data0 !== "string") {
              const err2 = { instancePath: instancePath + "/state", schemaPath: "#/oneOf/0/properties/state/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("complete" !== data0) {
              const err3 = { instancePath: instancePath + "/state", schemaPath: "#/oneOf/0/properties/state/const", keyword: "const", params: { allowedValue: "complete" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
          }
        }
      }
    } else {
      const err4 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs6 = errors;
  if (errors === _errs6) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing1;
      if (data.state === void 0 && (missing1 = "state") || data.reasons === void 0 && (missing1 = "reasons")) {
        const err5 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      } else {
        const _errs8 = errors;
        for (const key1 in data) {
          if (!(key1 === "downloadProgressFraction" || key1 === "reasons" || key1 === "recovery" || key1 === "state")) {
            const err6 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err6];
            } else {
              vErrors.push(err6);
            }
            errors++;
            break;
          }
        }
        if (_errs8 === errors) {
          if (data.downloadProgressFraction !== void 0) {
            let data1 = data.downloadProgressFraction;
            const _errs9 = errors;
            if (errors === _errs9) {
              if (typeof data1 == "number" && isFinite(data1)) {
                if (data1 > 17976931348623157e292 || isNaN(data1)) {
                  const err7 = { instancePath: instancePath + "/downloadProgressFraction", schemaPath: "#/oneOf/1/properties/downloadProgressFraction/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                } else {
                  if (data1 < 0 || isNaN(data1)) {
                    const err8 = { instancePath: instancePath + "/downloadProgressFraction", schemaPath: "#/oneOf/1/properties/downloadProgressFraction/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                    if (vErrors === null) {
                      vErrors = [err8];
                    } else {
                      vErrors.push(err8);
                    }
                    errors++;
                  } else {
                    if (data1 >= 1 || isNaN(data1)) {
                      const err9 = { instancePath: instancePath + "/downloadProgressFraction", schemaPath: "#/oneOf/1/properties/downloadProgressFraction/exclusiveMaximum", keyword: "exclusiveMaximum", params: { comparison: "<", limit: 1 }, message: "must be < 1" };
                      if (vErrors === null) {
                        vErrors = [err9];
                      } else {
                        vErrors.push(err9);
                      }
                      errors++;
                    }
                  }
                }
              } else {
                const err10 = { instancePath: instancePath + "/downloadProgressFraction", schemaPath: "#/oneOf/1/properties/downloadProgressFraction/type", keyword: "type", params: { type: "number" }, message: "must be number" };
                if (vErrors === null) {
                  vErrors = [err10];
                } else {
                  vErrors.push(err10);
                }
                errors++;
              }
            }
            var valid2 = _errs9 === errors;
          } else {
            var valid2 = true;
          }
          if (valid2) {
            if (data.reasons !== void 0) {
              let data2 = data.reasons;
              const _errs11 = errors;
              if (errors === _errs11) {
                if (Array.isArray(data2)) {
                  if (data2.length > 2) {
                    const err11 = { instancePath: instancePath + "/reasons", schemaPath: "#/oneOf/1/properties/reasons/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
                    if (vErrors === null) {
                      vErrors = [err11];
                    } else {
                      vErrors.push(err11);
                    }
                    errors++;
                  } else {
                    if (data2.length < 1) {
                      const err12 = { instancePath: instancePath + "/reasons", schemaPath: "#/oneOf/1/properties/reasons/minItems", keyword: "minItems", params: { limit: 1 }, message: "must NOT have fewer than 1 items" };
                      if (vErrors === null) {
                        vErrors = [err12];
                      } else {
                        vErrors.push(err12);
                      }
                      errors++;
                    } else {
                      var valid3 = true;
                      const len0 = data2.length;
                      for (let i0 = 0; i0 < len0; i0++) {
                        let data3 = data2[i0];
                        const _errs13 = errors;
                        if (typeof data3 !== "string") {
                          const err13 = { instancePath: instancePath + "/reasons/" + i0, schemaPath: "#/definitions/CatalogPartialReason/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                          if (vErrors === null) {
                            vErrors = [err13];
                          } else {
                            vErrors.push(err13);
                          }
                          errors++;
                        }
                        if (!(data3 === "part_file_present" || data3 === "expected_files_missing")) {
                          const err14 = { instancePath: instancePath + "/reasons/" + i0, schemaPath: "#/definitions/CatalogPartialReason/enum", keyword: "enum", params: { allowedValues: schema35.enum }, message: "must be equal to one of the allowed values" };
                          if (vErrors === null) {
                            vErrors = [err14];
                          } else {
                            vErrors.push(err14);
                          }
                          errors++;
                        }
                        var valid3 = _errs13 === errors;
                        if (!valid3) {
                          break;
                        }
                      }
                      if (valid3) {
                        let i1 = data2.length;
                        let j0;
                        if (i1 > 1) {
                          outer0: for (; i1--; ) {
                            for (j0 = i1; j0--; ) {
                              if (func0(data2[i1], data2[j0])) {
                                const err15 = { instancePath: instancePath + "/reasons", schemaPath: "#/oneOf/1/properties/reasons/uniqueItems", keyword: "uniqueItems", params: { i: i1, j: j0 }, message: "must NOT have duplicate items (items ## " + j0 + " and " + i1 + " are identical)" };
                                if (vErrors === null) {
                                  vErrors = [err15];
                                } else {
                                  vErrors.push(err15);
                                }
                                errors++;
                                break outer0;
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                } else {
                  const err16 = { instancePath: instancePath + "/reasons", schemaPath: "#/oneOf/1/properties/reasons/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                  if (vErrors === null) {
                    vErrors = [err16];
                  } else {
                    vErrors.push(err16);
                  }
                  errors++;
                }
              }
              var valid2 = _errs11 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.recovery !== void 0) {
                let data4 = data.recovery;
                const _errs16 = errors;
                const _errs17 = errors;
                if (errors === _errs17) {
                  if (data4 && typeof data4 == "object" && !Array.isArray(data4)) {
                    let missing2;
                    if (data4.recoveryToken === void 0 && (missing2 = "recoveryToken") || data4.repoId === void 0 && (missing2 = "repoId")) {
                      const err17 = { instancePath: instancePath + "/recovery", schemaPath: "#/definitions/CatalogRecoveryIdentity/required", keyword: "required", params: { missingProperty: missing2 }, message: "must have required property '" + missing2 + "'" };
                      if (vErrors === null) {
                        vErrors = [err17];
                      } else {
                        vErrors.push(err17);
                      }
                      errors++;
                    } else {
                      const _errs19 = errors;
                      for (const key2 in data4) {
                        if (!(key2 === "recoveryToken" || key2 === "repoId" || key2 === "selectedArtifactFiles" || key2 === "selectedArtifactId" || key2 === "selectedArtifactQuant")) {
                          const err18 = { instancePath: instancePath + "/recovery", schemaPath: "#/definitions/CatalogRecoveryIdentity/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
                          if (vErrors === null) {
                            vErrors = [err18];
                          } else {
                            vErrors.push(err18);
                          }
                          errors++;
                          break;
                        }
                      }
                      if (_errs19 === errors) {
                        if (data4.recoveryToken !== void 0) {
                          let data5 = data4.recoveryToken;
                          const _errs20 = errors;
                          if (errors === _errs20) {
                            if (typeof data5 === "string") {
                              if (!pattern0.test(data5)) {
                                const err19 = { instancePath: instancePath + "/recovery/recoveryToken", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/recoveryToken/pattern", keyword: "pattern", params: { pattern: "^v1:[0-9a-f]{64}$" }, message: 'must match pattern "^v1:[0-9a-f]{64}$"' };
                                if (vErrors === null) {
                                  vErrors = [err19];
                                } else {
                                  vErrors.push(err19);
                                }
                                errors++;
                              }
                            } else {
                              const err20 = { instancePath: instancePath + "/recovery/recoveryToken", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/recoveryToken/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                              if (vErrors === null) {
                                vErrors = [err20];
                              } else {
                                vErrors.push(err20);
                              }
                              errors++;
                            }
                          }
                          var valid7 = _errs20 === errors;
                        } else {
                          var valid7 = true;
                        }
                        if (valid7) {
                          if (data4.repoId !== void 0) {
                            let data6 = data4.repoId;
                            const _errs22 = errors;
                            if (errors === _errs22) {
                              if (typeof data6 === "string") {
                                if (func4(data6) > 96) {
                                  const err21 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/maxLength", keyword: "maxLength", params: { limit: 96 }, message: "must NOT have more than 96 characters" };
                                  if (vErrors === null) {
                                    vErrors = [err21];
                                  } else {
                                    vErrors.push(err21);
                                  }
                                  errors++;
                                } else {
                                  if (!pattern1.test(data6)) {
                                    const err22 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/pattern", keyword: "pattern", params: { pattern: "^(?!.*(?:--|\\.\\.))(?!.*\\.[gG][iI][tT]$)[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?/[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?$" }, message: 'must match pattern "^(?!.*(?:--|\\.\\.))(?!.*\\.[gG][iI][tT]$)[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?/[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?$"' };
                                    if (vErrors === null) {
                                      vErrors = [err22];
                                    } else {
                                      vErrors.push(err22);
                                    }
                                    errors++;
                                  } else {
                                    if (data6.length === 0 || pattern14.test(data6)) {
                                      const err23 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' };
                                      if (vErrors === null) {
                                        vErrors = [err23];
                                      } else {
                                        vErrors.push(err23);
                                      }
                                      errors++;
                                    } else {
                                      if (encodeURIComponent(data6).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        const err24 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' };
                                        if (vErrors === null) {
                                          vErrors = [err24];
                                        } else {
                                          vErrors.push(err24);
                                        }
                                        errors++;
                                      }
                                    }
                                  }
                                }
                              } else {
                                const err25 = { instancePath: instancePath + "/recovery/repoId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/repoId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                if (vErrors === null) {
                                  vErrors = [err25];
                                } else {
                                  vErrors.push(err25);
                                }
                                errors++;
                              }
                            }
                            var valid7 = _errs22 === errors;
                          } else {
                            var valid7 = true;
                          }
                          if (valid7) {
                            if (data4.selectedArtifactFiles !== void 0) {
                              let data7 = data4.selectedArtifactFiles;
                              const _errs24 = errors;
                              if (errors === _errs24) {
                                if (Array.isArray(data7)) {
                                  if (data7.length > 512) {
                                    const err26 = { instancePath: instancePath + "/recovery/selectedArtifactFiles", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/maxItems", keyword: "maxItems", params: { limit: 512 }, message: "must NOT have more than 512 items" };
                                    if (vErrors === null) {
                                      vErrors = [err26];
                                    } else {
                                      vErrors.push(err26);
                                    }
                                    errors++;
                                  } else {
                                    if (data7.length < 1) {
                                      const err27 = { instancePath: instancePath + "/recovery/selectedArtifactFiles", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/minItems", keyword: "minItems", params: { limit: 1 }, message: "must NOT have fewer than 1 items" };
                                      if (vErrors === null) {
                                        vErrors = [err27];
                                      } else {
                                        vErrors.push(err27);
                                      }
                                      errors++;
                                    } else {
                                      var valid8 = true;
                                      const len1 = data7.length;
                                      for (let i2 = 0; i2 < len1; i2++) {
                                        let data8 = data7[i2];
                                        const _errs26 = errors;
                                        if (errors === _errs26) {
                                          if (typeof data8 === "string") {
                                            if (data8.length === 0 || data8.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(data8) || Array.from(data8).some((letter) => letter.codePointAt(0) < 32 || letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159) || data8.split("/").some((component) => {
                                              const stem = component.split(".")[0].replace(/[a-z]/g, (letter) => letter.toUpperCase());
                                              return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g, "x").length > 255 || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
                                            })) {
                                              const err28 = { instancePath: instancePath + "/recovery/selectedArtifactFiles/" + i2, schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/items/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' };
                                              if (vErrors === null) {
                                                vErrors = [err28];
                                              } else {
                                                vErrors.push(err28);
                                              }
                                              errors++;
                                            } else {
                                              if (encodeURIComponent(data8).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                                const err29 = { instancePath: instancePath + "/recovery/selectedArtifactFiles/" + i2, schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/items/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' };
                                                if (vErrors === null) {
                                                  vErrors = [err29];
                                                } else {
                                                  vErrors.push(err29);
                                                }
                                                errors++;
                                              }
                                            }
                                          } else {
                                            const err30 = { instancePath: instancePath + "/recovery/selectedArtifactFiles/" + i2, schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/items/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                            if (vErrors === null) {
                                              vErrors = [err30];
                                            } else {
                                              vErrors.push(err30);
                                            }
                                            errors++;
                                          }
                                        }
                                        var valid8 = _errs26 === errors;
                                        if (!valid8) {
                                          break;
                                        }
                                      }
                                      if (valid8) {
                                        let i3 = data7.length;
                                        let j1;
                                        if (i3 > 1) {
                                          const indices0 = {};
                                          for (; i3--; ) {
                                            let item0 = data7[i3];
                                            if (typeof item0 !== "string") {
                                              continue;
                                            }
                                            if (typeof indices0[item0] == "number") {
                                              j1 = indices0[item0];
                                              const err31 = { instancePath: instancePath + "/recovery/selectedArtifactFiles", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/uniqueItems", keyword: "uniqueItems", params: { i: i3, j: j1 }, message: "must NOT have duplicate items (items ## " + j1 + " and " + i3 + " are identical)" };
                                              if (vErrors === null) {
                                                vErrors = [err31];
                                              } else {
                                                vErrors.push(err31);
                                              }
                                              errors++;
                                              break;
                                            }
                                            indices0[item0] = i3;
                                          }
                                        }
                                      }
                                    }
                                  }
                                } else {
                                  const err32 = { instancePath: instancePath + "/recovery/selectedArtifactFiles", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactFiles/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                                  if (vErrors === null) {
                                    vErrors = [err32];
                                  } else {
                                    vErrors.push(err32);
                                  }
                                  errors++;
                                }
                              }
                              var valid7 = _errs24 === errors;
                            } else {
                              var valid7 = true;
                            }
                            if (valid7) {
                              if (data4.selectedArtifactId !== void 0) {
                                let data9 = data4.selectedArtifactId;
                                const _errs28 = errors;
                                if (errors === _errs28) {
                                  if (typeof data9 === "string") {
                                    if (data9.length === 0 || pattern15.test(data9)) {
                                      const err33 = { instancePath: instancePath + "/recovery/selectedArtifactId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactId/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' };
                                      if (vErrors === null) {
                                        vErrors = [err33];
                                      } else {
                                        vErrors.push(err33);
                                      }
                                      errors++;
                                    } else {
                                      if (encodeURIComponent(data9).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        const err34 = { instancePath: instancePath + "/recovery/selectedArtifactId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' };
                                        if (vErrors === null) {
                                          vErrors = [err34];
                                        } else {
                                          vErrors.push(err34);
                                        }
                                        errors++;
                                      }
                                    }
                                  } else {
                                    const err35 = { instancePath: instancePath + "/recovery/selectedArtifactId", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                    if (vErrors === null) {
                                      vErrors = [err35];
                                    } else {
                                      vErrors.push(err35);
                                    }
                                    errors++;
                                  }
                                }
                                var valid7 = _errs28 === errors;
                              } else {
                                var valid7 = true;
                              }
                              if (valid7) {
                                if (data4.selectedArtifactQuant !== void 0) {
                                  let data10 = data4.selectedArtifactQuant;
                                  const _errs30 = errors;
                                  if (errors === _errs30) {
                                    if (typeof data10 === "string") {
                                      if (data10.length === 0 || pattern16.test(data10)) {
                                        const err36 = { instancePath: instancePath + "/recovery/selectedArtifactQuant", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactQuant/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' };
                                        if (vErrors === null) {
                                          vErrors = [err36];
                                        } else {
                                          vErrors.push(err36);
                                        }
                                        errors++;
                                      } else {
                                        if (encodeURIComponent(data10).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                          const err37 = { instancePath: instancePath + "/recovery/selectedArtifactQuant", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactQuant/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' };
                                          if (vErrors === null) {
                                            vErrors = [err37];
                                          } else {
                                            vErrors.push(err37);
                                          }
                                          errors++;
                                        }
                                      }
                                    } else {
                                      const err38 = { instancePath: instancePath + "/recovery/selectedArtifactQuant", schemaPath: "#/definitions/CatalogRecoveryIdentity/properties/selectedArtifactQuant/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                      if (vErrors === null) {
                                        vErrors = [err38];
                                      } else {
                                        vErrors.push(err38);
                                      }
                                      errors++;
                                    }
                                  }
                                  var valid7 = _errs30 === errors;
                                } else {
                                  var valid7 = true;
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  } else {
                    const err39 = { instancePath: instancePath + "/recovery", schemaPath: "#/definitions/CatalogRecoveryIdentity/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                    if (vErrors === null) {
                      vErrors = [err39];
                    } else {
                      vErrors.push(err39);
                    }
                    errors++;
                  }
                }
                var valid2 = _errs16 === errors;
              } else {
                var valid2 = true;
              }
              if (valid2) {
                if (data.state !== void 0) {
                  let data11 = data.state;
                  const _errs32 = errors;
                  if (typeof data11 !== "string") {
                    const err40 = { instancePath: instancePath + "/state", schemaPath: "#/oneOf/1/properties/state/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err40];
                    } else {
                      vErrors.push(err40);
                    }
                    errors++;
                  }
                  if ("partial" !== data11) {
                    const err41 = { instancePath: instancePath + "/state", schemaPath: "#/oneOf/1/properties/state/const", keyword: "const", params: { allowedValue: "partial" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err41];
                    } else {
                      vErrors.push(err41);
                    }
                    errors++;
                  }
                  var valid2 = _errs32 === errors;
                } else {
                  var valid2 = true;
                }
              }
            }
          }
        }
      }
    } else {
      const err42 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err42];
      } else {
        vErrors.push(err42);
      }
      errors++;
    }
  }
  var _valid0 = _errs6 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
  }
  if (!valid0) {
    const err43 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err43];
    } else {
      vErrors.push(err43);
    }
    errors++;
    validate28.errors = vErrors;
    return false;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate28.errors = vErrors;
  return errors === 0;
}
var pattern17 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern18 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern19 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern20 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern21 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern22 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern23 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate27(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.id === void 0 && (missing0 = "id") || data.modelDir === void 0 && (missing0 = "modelDir") || data.displayName === void 0 && (missing0 = "displayName") || data.modelType === void 0 && (missing0 = "modelType") || data.dependencyCount === void 0 && (missing0 = "dependencyCount") || data.relatedAvailable === void 0 && (missing0 = "relatedAvailable") || data.artifact === void 0 && (missing0 = "artifact") || data.integrity === void 0 && (missing0 = "integrity")) {
        validate27.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema33.properties, key0)) {
            validate27.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.artifact !== void 0) {
            const _errs2 = errors;
            if (!validate28(data.artifact, { instancePath: instancePath + "/artifact", parentData: data, parentDataProperty: "artifact", rootData })) {
              vErrors = vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
              errors = vErrors.length;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.dependencyCount !== void 0) {
              let data1 = data.dependencyCount;
              const _errs3 = errors;
              if (!(typeof data1 == "number" && (!(data1 % 1) && !isNaN(data1)) && isFinite(data1))) {
                validate27.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs3) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 512 || isNaN(data1)) {
                    validate27.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 512 }, message: "must be <= 512" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate27.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                      return false;
                    }
                  }
                }
              }
              var valid0 = _errs3 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.displayDate !== void 0) {
                let data2 = data.displayDate;
                const _errs5 = errors;
                if (errors === _errs5) {
                  if (typeof data2 === "string") {
                    if (func4(data2) < 1) {
                      validate27.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                      return false;
                    } else {
                      if (data2.length === 0 || pattern17.test(data2)) {
                        validate27.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                        return false;
                      } else {
                        if (encodeURIComponent(data2).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                          validate27.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate27.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                }
                var valid0 = _errs5 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.displayName !== void 0) {
                  let data3 = data.displayName;
                  const _errs7 = errors;
                  if (errors === _errs7) {
                    if (typeof data3 === "string") {
                      if (func4(data3) < 1) {
                        validate27.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                        return false;
                      } else {
                        if (data3.length === 0 || pattern18.test(data3)) {
                          validate27.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data3).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate27.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate27.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                      return false;
                    }
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.format !== void 0) {
                    let data4 = data.format;
                    const _errs9 = errors;
                    if (errors === _errs9) {
                      if (typeof data4 === "string") {
                        if (func4(data4) < 1) {
                          validate27.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                          return false;
                        } else {
                          if (data4.length === 0 || pattern19.test(data4)) {
                            validate27.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                            return false;
                          } else {
                            if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                              validate27.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate27.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                    }
                    var valid0 = _errs9 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.id !== void 0) {
                      let data5 = data.id;
                      const _errs11 = errors;
                      if (errors === _errs11) {
                        if (typeof data5 === "string") {
                          if (func4(data5) < 1) {
                            validate27.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                            return false;
                          } else {
                            if (data5.length === 0 || pattern20.test(data5)) {
                              validate27.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                              return false;
                            } else {
                              if (encodeURIComponent(data5).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                validate27.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate27.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                          return false;
                        }
                      }
                      var valid0 = _errs11 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.integrity !== void 0) {
                        let data6 = data.integrity;
                        const _errs13 = errors;
                        const _errs15 = errors;
                        let valid2 = false;
                        let passing0 = null;
                        const _errs16 = errors;
                        if (errors === _errs16) {
                          if (data6 && typeof data6 == "object" && !Array.isArray(data6)) {
                            let missing1;
                            if (data6.state === void 0 && (missing1 = "state")) {
                              const err0 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
                              if (vErrors === null) {
                                vErrors = [err0];
                              } else {
                                vErrors.push(err0);
                              }
                              errors++;
                            } else {
                              const _errs18 = errors;
                              for (const key1 in data6) {
                                if (!(key1 === "state")) {
                                  const err1 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
                                  if (vErrors === null) {
                                    vErrors = [err1];
                                  } else {
                                    vErrors.push(err1);
                                  }
                                  errors++;
                                  break;
                                }
                              }
                              if (_errs18 === errors) {
                                if (data6.state !== void 0) {
                                  let data7 = data6.state;
                                  if (typeof data7 !== "string") {
                                    const err2 = { instancePath: instancePath + "/integrity/state", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/properties/state/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                    if (vErrors === null) {
                                      vErrors = [err2];
                                    } else {
                                      vErrors.push(err2);
                                    }
                                    errors++;
                                  }
                                  if ("clean" !== data7) {
                                    const err3 = { instancePath: instancePath + "/integrity/state", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/properties/state/const", keyword: "const", params: { allowedValue: "clean" }, message: "must be equal to constant" };
                                    if (vErrors === null) {
                                      vErrors = [err3];
                                    } else {
                                      vErrors.push(err3);
                                    }
                                    errors++;
                                  }
                                }
                              }
                            }
                          } else {
                            const err4 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                            if (vErrors === null) {
                              vErrors = [err4];
                            } else {
                              vErrors.push(err4);
                            }
                            errors++;
                          }
                        }
                        var _valid0 = _errs16 === errors;
                        if (_valid0) {
                          valid2 = true;
                          passing0 = 0;
                        }
                        const _errs21 = errors;
                        if (errors === _errs21) {
                          if (data6 && typeof data6 == "object" && !Array.isArray(data6)) {
                            let missing2;
                            if (data6.state === void 0 && (missing2 = "state") || data6.count === void 0 && (missing2 = "count") || data6.otherModelIds === void 0 && (missing2 = "otherModelIds")) {
                              const err5 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/required", keyword: "required", params: { missingProperty: missing2 }, message: "must have required property '" + missing2 + "'" };
                              if (vErrors === null) {
                                vErrors = [err5];
                              } else {
                                vErrors.push(err5);
                              }
                              errors++;
                            } else {
                              const _errs23 = errors;
                              for (const key2 in data6) {
                                if (!(key2 === "count" || key2 === "otherModelIds" || key2 === "state")) {
                                  const err6 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
                                  if (vErrors === null) {
                                    vErrors = [err6];
                                  } else {
                                    vErrors.push(err6);
                                  }
                                  errors++;
                                  break;
                                }
                              }
                              if (_errs23 === errors) {
                                if (data6.count !== void 0) {
                                  let data8 = data6.count;
                                  const _errs24 = errors;
                                  if (!(typeof data8 == "number" && (!(data8 % 1) && !isNaN(data8)) && isFinite(data8))) {
                                    const err7 = { instancePath: instancePath + "/integrity/count", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/count/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
                                    if (vErrors === null) {
                                      vErrors = [err7];
                                    } else {
                                      vErrors.push(err7);
                                    }
                                    errors++;
                                  }
                                  if (errors === _errs24) {
                                    if (typeof data8 == "number" && isFinite(data8)) {
                                      if (data8 > 4294967295 || isNaN(data8)) {
                                        const err8 = { instancePath: instancePath + "/integrity/count", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" };
                                        if (vErrors === null) {
                                          vErrors = [err8];
                                        } else {
                                          vErrors.push(err8);
                                        }
                                        errors++;
                                      } else {
                                        if (data8 < 0 || isNaN(data8)) {
                                          const err9 = { instancePath: instancePath + "/integrity/count", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                                          if (vErrors === null) {
                                            vErrors = [err9];
                                          } else {
                                            vErrors.push(err9);
                                          }
                                          errors++;
                                        }
                                      }
                                    }
                                  }
                                  var valid4 = _errs24 === errors;
                                } else {
                                  var valid4 = true;
                                }
                                if (valid4) {
                                  if (data6.otherModelIds !== void 0) {
                                    let data9 = data6.otherModelIds;
                                    const _errs26 = errors;
                                    if (errors === _errs26) {
                                      if (Array.isArray(data9)) {
                                        var valid5 = true;
                                        const len0 = data9.length;
                                        for (let i0 = 0; i0 < len0; i0++) {
                                          const _errs28 = errors;
                                          if (typeof data9[i0] !== "string") {
                                            const err10 = { instancePath: instancePath + "/integrity/otherModelIds/" + i0, schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/otherModelIds/items/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                            if (vErrors === null) {
                                              vErrors = [err10];
                                            } else {
                                              vErrors.push(err10);
                                            }
                                            errors++;
                                          }
                                          var valid5 = _errs28 === errors;
                                          if (!valid5) {
                                            break;
                                          }
                                        }
                                      } else {
                                        const err11 = { instancePath: instancePath + "/integrity/otherModelIds", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/otherModelIds/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                                        if (vErrors === null) {
                                          vErrors = [err11];
                                        } else {
                                          vErrors.push(err11);
                                        }
                                        errors++;
                                      }
                                    }
                                    var valid4 = _errs26 === errors;
                                  } else {
                                    var valid4 = true;
                                  }
                                  if (valid4) {
                                    if (data6.state !== void 0) {
                                      let data11 = data6.state;
                                      const _errs30 = errors;
                                      if (typeof data11 !== "string") {
                                        const err12 = { instancePath: instancePath + "/integrity/state", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/state/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                        if (vErrors === null) {
                                          vErrors = [err12];
                                        } else {
                                          vErrors.push(err12);
                                        }
                                        errors++;
                                      }
                                      if ("duplicate" !== data11) {
                                        const err13 = { instancePath: instancePath + "/integrity/state", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/properties/state/const", keyword: "const", params: { allowedValue: "duplicate" }, message: "must be equal to constant" };
                                        if (vErrors === null) {
                                          vErrors = [err13];
                                        } else {
                                          vErrors.push(err13);
                                        }
                                        errors++;
                                      }
                                      var valid4 = _errs30 === errors;
                                    } else {
                                      var valid4 = true;
                                    }
                                  }
                                }
                              }
                            }
                          } else {
                            const err14 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                            if (vErrors === null) {
                              vErrors = [err14];
                            } else {
                              vErrors.push(err14);
                            }
                            errors++;
                          }
                        }
                        var _valid0 = _errs21 === errors;
                        if (_valid0 && valid2) {
                          valid2 = false;
                          passing0 = [passing0, 1];
                        } else {
                          if (_valid0) {
                            valid2 = true;
                            passing0 = 1;
                          }
                        }
                        if (!valid2) {
                          const err15 = { instancePath: instancePath + "/integrity", schemaPath: "#/definitions/CatalogIntegrityState/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
                          if (vErrors === null) {
                            vErrors = [err15];
                          } else {
                            vErrors.push(err15);
                          }
                          errors++;
                          validate27.errors = vErrors;
                          return false;
                        } else {
                          errors = _errs15;
                          if (vErrors !== null) {
                            if (_errs15) {
                              vErrors.length = _errs15;
                            } else {
                              vErrors = null;
                            }
                          }
                        }
                        var valid0 = _errs13 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.modelDir !== void 0) {
                          let data12 = data.modelDir;
                          const _errs32 = errors;
                          if (errors === _errs32) {
                            if (typeof data12 === "string") {
                              if (func4(data12) < 1) {
                                validate27.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                return false;
                              } else {
                                if (data12.length === 0 || pattern21.test(data12)) {
                                  validate27.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                  return false;
                                } else {
                                  if (encodeURIComponent(data12).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                    validate27.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                    return false;
                                  }
                                }
                              }
                            } else {
                              validate27.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                              return false;
                            }
                          }
                          var valid0 = _errs32 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.modelType !== void 0) {
                            let data13 = data.modelType;
                            const _errs34 = errors;
                            if (errors === _errs34) {
                              if (typeof data13 === "string") {
                                if (func4(data13) < 1) {
                                  validate27.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                  return false;
                                } else {
                                  if (data13.length === 0 || pattern22.test(data13)) {
                                    validate27.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                    return false;
                                  } else {
                                    if (encodeURIComponent(data13).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                      validate27.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                      return false;
                                    }
                                  }
                                }
                              } else {
                                validate27.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                return false;
                              }
                            }
                            var valid0 = _errs34 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.quantization !== void 0) {
                              let data14 = data.quantization;
                              const _errs36 = errors;
                              if (errors === _errs36) {
                                if (typeof data14 === "string") {
                                  if (func4(data14) < 1) {
                                    validate27.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                    return false;
                                  } else {
                                    if (data14.length === 0 || pattern23.test(data14)) {
                                      validate27.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                      return false;
                                    } else {
                                      if (encodeURIComponent(data14).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        validate27.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                        return false;
                                      }
                                    }
                                  }
                                } else {
                                  validate27.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                  return false;
                                }
                              }
                              var valid0 = _errs36 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.relatedAvailable !== void 0) {
                                const _errs38 = errors;
                                if (typeof data.relatedAvailable !== "boolean") {
                                  validate27.errors = [{ instancePath: instancePath + "/relatedAvailable", schemaPath: "#/properties/relatedAvailable/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                  return false;
                                }
                                var valid0 = _errs38 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.sizeBytes !== void 0) {
                                  let data16 = data.sizeBytes;
                                  const _errs40 = errors;
                                  if (!(typeof data16 == "number" && (!(data16 % 1) && !isNaN(data16)) && isFinite(data16))) {
                                    validate27.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                    return false;
                                  }
                                  if (errors === _errs40) {
                                    if (typeof data16 == "number" && isFinite(data16)) {
                                      if (data16 > 9007199254740991 || isNaN(data16)) {
                                        validate27.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                        return false;
                                      } else {
                                        if (data16 < 0 || isNaN(data16)) {
                                          validate27.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                          return false;
                                        }
                                      }
                                    }
                                  }
                                  var valid0 = _errs40 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.integrity?.state === "duplicate" && (!Array.isArray(data.integrity.otherModelIds) || data.integrity.count !== data.integrity.otherModelIds.length + 1 || data.integrity.count < 2 || data.integrity.otherModelIds.includes(data.id) || new Set(data.integrity.otherModelIds).size !== data.integrity.otherModelIds.length)) {
                                    validate27.errors = [{ instancePath, schemaPath: "#/pumasCatalogRow", keyword: "pumasCatalogRow", params: {}, message: 'must pass "pumasCatalogRow" keyword validation' }];
                                    return false;
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate27.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate27.errors = vErrors;
  return errors === 0;
}
function validate26(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.models === void 0 && (missing0 = "models")) {
        validate26.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "models" || key0 === "success")) {
            validate26.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.models !== void 0) {
            let data0 = data.models;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data0 && typeof data0 == "object" && !Array.isArray(data0)) {
                const _errs4 = errors;
                for (const key1 in data0) {
                  const _errs5 = errors;
                  if (!validate27(data0[key1], { instancePath: instancePath + "/models/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data0, parentDataProperty: key1, rootData })) {
                    vErrors = vErrors === null ? validate27.errors : vErrors.concat(validate27.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs5 === errors;
                  if (!valid1) {
                    break;
                  }
                }
                if (_errs4 === errors) {
                  if (Object.entries(data0).some(([key, value]) => value === null || typeof value !== "object" || key !== value.id)) {
                    validate26.errors = [{ instancePath: instancePath + "/models", schemaPath: "#/properties/models/pumasCatalogMap", keyword: "pumasCatalogMap", params: {}, message: 'must pass "pumasCatalogMap" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate26.errors = [{ instancePath: instancePath + "/models", schemaPath: "#/properties/models/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.success !== void 0) {
              let data2 = data.success;
              const _errs6 = errors;
              if (typeof data2 !== "boolean") {
                validate26.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate26.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                return false;
              }
              var valid0 = _errs6 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate26.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate26.errors = vErrors;
  return errors === 0;
}
var validatePartialDownloadOutcome = validate31;
var schema38 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "definitions": { "DownloadStatus": { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" }, "PartialDownloadActionName": { "enum": ["resume", "recover", "attach", "none"], "type": "string" }, "PartialDownloadReason": { "enum": ["hf_client_unavailable", "download_root_busy", "model_not_found", "model_not_partial", "recovery_unavailable", "recovery_context_stale", "resume_rejected", "already_completed", "already_cancelled", "invalid_repo_id", "repo_not_found", "rate_limited", "permission_denied", "network_error", "recover_failed"], "type": "string" } }, "properties": { "action": { "$ref": "#/definitions/PartialDownloadActionName" }, "download_id": { "type": ["string", "null"] }, "error": { "type": ["string", "null"] }, "reason_code": { "anyOf": [{ "$ref": "#/definitions/PartialDownloadReason" }, { "type": "null" }] }, "status": { "anyOf": [{ "$ref": "#/definitions/DownloadStatus" }, { "type": "null" }] }, "success": { "type": "boolean" } }, "pumasPartialOutcome": true, "required": ["success", "action", "download_id", "status", "reason_code", "error"], "title": "PartialDownloadOutcome", "type": "object" };
var schema39 = { "enum": ["resume", "recover", "attach", "none"], "type": "string" };
var schema40 = { "enum": ["hf_client_unavailable", "download_root_busy", "model_not_found", "model_not_partial", "recovery_unavailable", "recovery_context_stale", "resume_rejected", "already_completed", "already_cancelled", "invalid_repo_id", "repo_not_found", "rate_limited", "permission_denied", "network_error", "recover_failed"], "type": "string" };
var schema41 = { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" };
function validate31(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.action === void 0 && (missing0 = "action") || data.download_id === void 0 && (missing0 = "download_id") || data.status === void 0 && (missing0 = "status") || data.reason_code === void 0 && (missing0 = "reason_code") || data.error === void 0 && (missing0 = "error")) {
        validate31.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "action" || key0 === "download_id" || key0 === "error" || key0 === "reason_code" || key0 === "status" || key0 === "success")) {
            validate31.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.action !== void 0) {
            let data0 = data.action;
            const _errs2 = errors;
            if (typeof data0 !== "string") {
              validate31.errors = [{ instancePath: instancePath + "/action", schemaPath: "#/definitions/PartialDownloadActionName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            if (!(data0 === "resume" || data0 === "recover" || data0 === "attach" || data0 === "none")) {
              validate31.errors = [{ instancePath: instancePath + "/action", schemaPath: "#/definitions/PartialDownloadActionName/enum", keyword: "enum", params: { allowedValues: schema39.enum }, message: "must be equal to one of the allowed values" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.download_id !== void 0) {
              let data1 = data.download_id;
              const _errs5 = errors;
              if (typeof data1 !== "string" && data1 !== null) {
                validate31.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/type", keyword: "type", params: { type: schema38.properties.download_id.type }, message: "must be string,null" }];
                return false;
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.error !== void 0) {
                let data2 = data.error;
                const _errs7 = errors;
                if (typeof data2 !== "string" && data2 !== null) {
                  validate31.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema38.properties.error.type }, message: "must be string,null" }];
                  return false;
                }
                var valid0 = _errs7 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.reason_code !== void 0) {
                  let data3 = data.reason_code;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  let valid2 = false;
                  const _errs11 = errors;
                  if (typeof data3 !== "string") {
                    const err0 = { instancePath: instancePath + "/reason_code", schemaPath: "#/definitions/PartialDownloadReason/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err0];
                    } else {
                      vErrors.push(err0);
                    }
                    errors++;
                  }
                  if (!(data3 === "hf_client_unavailable" || data3 === "download_root_busy" || data3 === "model_not_found" || data3 === "model_not_partial" || data3 === "recovery_unavailable" || data3 === "recovery_context_stale" || data3 === "resume_rejected" || data3 === "already_completed" || data3 === "already_cancelled" || data3 === "invalid_repo_id" || data3 === "repo_not_found" || data3 === "rate_limited" || data3 === "permission_denied" || data3 === "network_error" || data3 === "recover_failed")) {
                    const err1 = { instancePath: instancePath + "/reason_code", schemaPath: "#/definitions/PartialDownloadReason/enum", keyword: "enum", params: { allowedValues: schema40.enum }, message: "must be equal to one of the allowed values" };
                    if (vErrors === null) {
                      vErrors = [err1];
                    } else {
                      vErrors.push(err1);
                    }
                    errors++;
                  }
                  var _valid0 = _errs11 === errors;
                  valid2 = valid2 || _valid0;
                  if (!valid2) {
                    const _errs14 = errors;
                    if (data3 !== null) {
                      const err2 = { instancePath: instancePath + "/reason_code", schemaPath: "#/properties/reason_code/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
                      if (vErrors === null) {
                        vErrors = [err2];
                      } else {
                        vErrors.push(err2);
                      }
                      errors++;
                    }
                    var _valid0 = _errs14 === errors;
                    valid2 = valid2 || _valid0;
                  }
                  if (!valid2) {
                    const err3 = { instancePath: instancePath + "/reason_code", schemaPath: "#/properties/reason_code/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
                    if (vErrors === null) {
                      vErrors = [err3];
                    } else {
                      vErrors.push(err3);
                    }
                    errors++;
                    validate31.errors = vErrors;
                    return false;
                  } else {
                    errors = _errs10;
                    if (vErrors !== null) {
                      if (_errs10) {
                        vErrors.length = _errs10;
                      } else {
                        vErrors = null;
                      }
                    }
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.status !== void 0) {
                    let data4 = data.status;
                    const _errs16 = errors;
                    const _errs17 = errors;
                    let valid4 = false;
                    const _errs18 = errors;
                    if (typeof data4 !== "string") {
                      const err4 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err4];
                      } else {
                        vErrors.push(err4);
                      }
                      errors++;
                    }
                    if (!(data4 === "queued" || data4 === "downloading" || data4 === "pausing" || data4 === "paused" || data4 === "cancelling" || data4 === "completed" || data4 === "cancelled" || data4 === "error")) {
                      const err5 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/enum", keyword: "enum", params: { allowedValues: schema41.enum }, message: "must be equal to one of the allowed values" };
                      if (vErrors === null) {
                        vErrors = [err5];
                      } else {
                        vErrors.push(err5);
                      }
                      errors++;
                    }
                    var _valid1 = _errs18 === errors;
                    valid4 = valid4 || _valid1;
                    if (!valid4) {
                      const _errs21 = errors;
                      if (data4 !== null) {
                        const err6 = { instancePath: instancePath + "/status", schemaPath: "#/properties/status/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
                        if (vErrors === null) {
                          vErrors = [err6];
                        } else {
                          vErrors.push(err6);
                        }
                        errors++;
                      }
                      var _valid1 = _errs21 === errors;
                      valid4 = valid4 || _valid1;
                    }
                    if (!valid4) {
                      const err7 = { instancePath: instancePath + "/status", schemaPath: "#/properties/status/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
                      if (vErrors === null) {
                        vErrors = [err7];
                      } else {
                        vErrors.push(err7);
                      }
                      errors++;
                      validate31.errors = vErrors;
                      return false;
                    } else {
                      errors = _errs17;
                      if (vErrors !== null) {
                        if (_errs17) {
                          vErrors.length = _errs17;
                        } else {
                          vErrors = null;
                        }
                      }
                    }
                    var valid0 = _errs16 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.success !== void 0) {
                      const _errs23 = errors;
                      if (typeof data.success !== "boolean") {
                        validate31.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                        return false;
                      }
                      var valid0 = _errs23 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (!((data.action === "resume" || data.action === "recover") && data.success === true && typeof data.download_id === "string" && data.download_id.length > 0 && data.status === "queued" && data.reason_code === null && data.error === null || data.action === "attach" && data.success === true && typeof data.download_id === "string" && data.download_id.length > 0 && ["queued", "downloading", "pausing", "cancelling"].includes(data.status) && data.reason_code === null && data.error === null || data.action === "none" && data.success === false && typeof data.error === "string" && (data.download_id === null && data.status === null && !["already_completed", "already_cancelled", "resume_rejected"].includes(data.reason_code) && data.reason_code !== null || typeof data.download_id === "string" && data.download_id.length > 0 && (data.status === "completed" && data.reason_code === "already_completed" || data.status === "cancelled" && data.reason_code === "already_cancelled" || ["paused", "error"].includes(data.status) && data.reason_code === "resume_rejected")))) {
                        validate31.errors = [{ instancePath, schemaPath: "#/pumasPartialOutcome", keyword: "pumasPartialOutcome", params: {}, message: 'must pass "pumasPartialOutcome" keyword validation' }];
                        return false;
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate31.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate31.errors = vErrors;
  return errors === 0;
}
var validatePublicError = validate32;
var schema43 = { "description": "Stable public failure categories shared by RPC transports.", "enum": ["invalid_request", "not_found", "conflict", "cancelled", "unavailable", "operation_failed", "internal"], "type": "string" };
function validate32(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.code === void 0 && (missing0 = "code") || data.class === void 0 && (missing0 = "class") || data.message === void 0 && (missing0 = "message")) {
        validate32.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "class" || key0 === "code" || key0 === "message")) {
            validate32.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.class !== void 0) {
            let data0 = data.class;
            const _errs2 = errors;
            if (typeof data0 !== "string") {
              validate32.errors = [{ instancePath: instancePath + "/class", schemaPath: "#/definitions/PublicErrorClass/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            if (!(data0 === "invalid_request" || data0 === "not_found" || data0 === "conflict" || data0 === "cancelled" || data0 === "unavailable" || data0 === "operation_failed" || data0 === "internal")) {
              validate32.errors = [{ instancePath: instancePath + "/class", schemaPath: "#/definitions/PublicErrorClass/enum", keyword: "enum", params: { allowedValues: schema43.enum }, message: "must be equal to one of the allowed values" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.code !== void 0) {
              let data1 = data.code;
              const _errs5 = errors;
              if (!(typeof data1 == "number" && (!(data1 % 1) && !isNaN(data1)) && isFinite(data1))) {
                validate32.errors = [{ instancePath: instancePath + "/code", schemaPath: "#/properties/code/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs5) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 2147483647 || isNaN(data1)) {
                    validate32.errors = [{ instancePath: instancePath + "/code", schemaPath: "#/properties/code/maximum", keyword: "maximum", params: { comparison: "<=", limit: 2147483647 }, message: "must be <= 2147483647" }];
                    return false;
                  } else {
                    if (data1 < -2147483648 || isNaN(data1)) {
                      validate32.errors = [{ instancePath: instancePath + "/code", schemaPath: "#/properties/code/minimum", keyword: "minimum", params: { comparison: ">=", limit: -2147483648 }, message: "must be >= -2147483648" }];
                      return false;
                    }
                  }
                }
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.message !== void 0) {
                const _errs7 = errors;
                if (typeof data.message !== "string") {
                  validate32.errors = [{ instancePath: instancePath + "/message", schemaPath: "#/properties/message/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                  return false;
                }
                var valid0 = _errs7 === errors;
              } else {
                var valid0 = true;
              }
            }
          }
        }
      }
    } else {
      validate32.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate32.errors = vErrors;
  return errors === 0;
}
var validateRecoverDownloadParams = validate33;
function validate33(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.modelId === void 0 && (missing0 = "modelId") || data.recoveryToken === void 0 && (missing0 = "recoveryToken")) {
        validate33.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "modelId" || key0 === "recoveryToken")) {
            validate33.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.modelId !== void 0) {
            let data0 = data.modelId;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (typeof data0 === "string") {
                if (data0.length === 0 || data0.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(data0) || Array.from(data0).some((letter) => letter.codePointAt(0) < 32 || letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159) || data0.split("/").some((component) => {
                  const stem = component.split(".")[0].replace(/[a-z]/g, (letter) => letter.toUpperCase());
                  return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g, "x").length > 255 || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
                })) {
                  validate33.errors = [{ instancePath: instancePath + "/modelId", schemaPath: "#/properties/modelId/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' }];
                  return false;
                } else {
                  if (encodeURIComponent(data0).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                    validate33.errors = [{ instancePath: instancePath + "/modelId", schemaPath: "#/properties/modelId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate33.errors = [{ instancePath: instancePath + "/modelId", schemaPath: "#/properties/modelId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.recoveryToken !== void 0) {
              let data1 = data.recoveryToken;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (typeof data1 === "string") {
                  if (!pattern0.test(data1)) {
                    validate33.errors = [{ instancePath: instancePath + "/recoveryToken", schemaPath: "#/properties/recoveryToken/pattern", keyword: "pattern", params: { pattern: "^v1:[0-9a-f]{64}$" }, message: 'must match pattern "^v1:[0-9a-f]{64}$"' }];
                    return false;
                  }
                } else {
                  validate33.errors = [{ instancePath: instancePath + "/recoveryToken", schemaPath: "#/properties/recoveryToken/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                  return false;
                }
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate33.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate33.errors = vErrors;
  return errors === 0;
}
var validateSearchCatalogParams = validate34;
var schema45 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "properties": { "limit": { "maximum": 512, "minimum": 1, "type": ["integer", "null"] }, "offset": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "query": { "pumasUtf8Max": 4096, "type": "string" } }, "required": ["query"], "title": "SearchCatalogParams", "type": "object" };
function validate34(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.query === void 0 && (missing0 = "query")) {
        validate34.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "limit" || key0 === "offset" || key0 === "query")) {
            validate34.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.limit !== void 0) {
            let data0 = data.limit;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0)) && data0 !== null) {
              validate34.errors = [{ instancePath: instancePath + "/limit", schemaPath: "#/properties/limit/type", keyword: "type", params: { type: schema45.properties.limit.type }, message: "must be integer,null" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 512 || isNaN(data0)) {
                  validate34.errors = [{ instancePath: instancePath + "/limit", schemaPath: "#/properties/limit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 512 }, message: "must be <= 512" }];
                  return false;
                } else {
                  if (data0 < 1 || isNaN(data0)) {
                    validate34.errors = [{ instancePath: instancePath + "/limit", schemaPath: "#/properties/limit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
                    return false;
                  }
                }
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.offset !== void 0) {
              let data1 = data.offset;
              const _errs4 = errors;
              if (!(typeof data1 == "number" && (!(data1 % 1) && !isNaN(data1)) && isFinite(data1)) && data1 !== null) {
                validate34.errors = [{ instancePath: instancePath + "/offset", schemaPath: "#/properties/offset/type", keyword: "type", params: { type: schema45.properties.offset.type }, message: "must be integer,null" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 4294967295 || isNaN(data1)) {
                    validate34.errors = [{ instancePath: instancePath + "/offset", schemaPath: "#/properties/offset/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate34.errors = [{ instancePath: instancePath + "/offset", schemaPath: "#/properties/offset/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                      return false;
                    }
                  }
                }
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.query !== void 0) {
                let data2 = data.query;
                const _errs6 = errors;
                if (errors === _errs6) {
                  if (typeof data2 === "string") {
                    if (encodeURIComponent(data2).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                      validate34.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                      return false;
                    }
                  } else {
                    validate34.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
            }
          }
        }
      }
    } else {
      validate34.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate34.errors = vErrors;
  return errors === 0;
}
export {
  validateCatalogSearchOutcome,
  validateDownloadIdParams,
  validateDownloadListOutcome,
  validateDownloadMutationOutcome,
  validateDownloadStartedOutcome,
  validateDownloadStatusOutcome,
  validateLinkHealthOutcome,
  validateModelIndexRefreshOutcome,
  validateModelsOutcome,
  validatePartialDownloadOutcome,
  validatePublicError,
  validateRecoverDownloadParams,
  validateSearchCatalogParams
};
