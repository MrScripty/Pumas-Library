// Generated from pumas-rpc contract.rs; SHA256 860c21e5455db063e373b1b2e996503dd4985abad8456e80fec38bf4bfd94baf. DO NOT EDIT.
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function __require() {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

// ../node_modules/.pnpm/ajv@8.20.0/node_modules/ajv/dist/runtime/ucs2length.js
var require_ucs2length = __commonJS({
  "../node_modules/.pnpm/ajv@8.20.0/node_modules/ajv/dist/runtime/ucs2length.js"(exports) {
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

// ../node_modules/.pnpm/fast-deep-equal@3.1.3/node_modules/fast-deep-equal/index.js
var require_fast_deep_equal = __commonJS({
  "../node_modules/.pnpm/fast-deep-equal@3.1.3/node_modules/fast-deep-equal/index.js"(exports, module) {
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

// ../node_modules/.pnpm/ajv@8.20.0/node_modules/ajv/dist/runtime/equal.js
var require_equal = __commonJS({
  "../node_modules/.pnpm/ajv@8.20.0/node_modules/ajv/dist/runtime/equal.js"(exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    var equal = require_fast_deep_equal();
    equal.code = 'require("ajv/dist/runtime/equal").default';
    exports.default = equal;
  }
});

// desktop-contract.validators.js
var validateBackendStatusOutcome = validate10;
function validate11(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.backend === void 0 && (missing0 = "backend") || data.name === void 0 && (missing0 = "name") || data.ready === void 0 && (missing0 = "ready")) {
        validate11.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backend" || key0 === "name" || key0 === "ready")) {
            validate11.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.backend !== void 0) {
            let data0 = data.backend;
            const _errs2 = errors;
            const _errs5 = errors;
            let valid3 = false;
            let passing0 = null;
            const _errs6 = errors;
            if (typeof data0 !== "string") {
              const err0 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err0];
              } else {
                vErrors.push(err0);
              }
              errors++;
            }
            if ("python_conversion" !== data0) {
              const err1 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/0/const", keyword: "const", params: { allowedValue: "python_conversion" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err1];
              } else {
                vErrors.push(err1);
              }
              errors++;
            }
            var _valid0 = _errs6 === errors;
            if (_valid0) {
              valid3 = true;
              passing0 = 0;
            }
            const _errs8 = errors;
            if (typeof data0 !== "string") {
              const err2 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("llama_cpp" !== data0) {
              const err3 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/1/const", keyword: "const", params: { allowedValue: "llama_cpp" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
            var _valid0 = _errs8 === errors;
            if (_valid0 && valid3) {
              valid3 = false;
              passing0 = [passing0, 1];
            } else {
              if (_valid0) {
                valid3 = true;
                passing0 = 1;
              }
              const _errs10 = errors;
              if (typeof data0 !== "string") {
                const err4 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err4];
                } else {
                  vErrors.push(err4);
                }
                errors++;
              }
              if ("nvfp4" !== data0) {
                const err5 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/2/const", keyword: "const", params: { allowedValue: "nvfp4" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err5];
                } else {
                  vErrors.push(err5);
                }
                errors++;
              }
              var _valid0 = _errs10 === errors;
              if (_valid0 && valid3) {
                valid3 = false;
                passing0 = [passing0, 2];
              } else {
                if (_valid0) {
                  valid3 = true;
                  passing0 = 2;
                }
                const _errs12 = errors;
                if (typeof data0 !== "string") {
                  const err6 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err6];
                  } else {
                    vErrors.push(err6);
                  }
                  errors++;
                }
                if ("sherry" !== data0) {
                  const err7 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/3/const", keyword: "const", params: { allowedValue: "sherry" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                }
                var _valid0 = _errs12 === errors;
                if (_valid0 && valid3) {
                  valid3 = false;
                  passing0 = [passing0, 3];
                } else {
                  if (_valid0) {
                    valid3 = true;
                    passing0 = 3;
                  }
                }
              }
            }
            if (!valid3) {
              const err8 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
              if (vErrors === null) {
                vErrors = [err8];
              } else {
                vErrors.push(err8);
              }
              errors++;
              validate11.errors = vErrors;
              return false;
            } else {
              errors = _errs5;
              if (vErrors !== null) {
                if (_errs5) {
                  vErrors.length = _errs5;
                } else {
                  vErrors = null;
                }
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.name !== void 0) {
              const _errs14 = errors;
              if (typeof data.name !== "string") {
                validate11.errors = [{ instancePath: instancePath + "/name", schemaPath: "#/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
              var valid0 = _errs14 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.ready !== void 0) {
                const _errs16 = errors;
                if (typeof data.ready !== "boolean") {
                  validate11.errors = [{ instancePath: instancePath + "/ready", schemaPath: "#/properties/ready/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                  return false;
                }
                var valid0 = _errs16 === errors;
              } else {
                var valid0 = true;
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
      if (data.success === void 0 && (missing0 = "success") || data.backends === void 0 && (missing0 = "backends")) {
        validate10.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backends" || key0 === "success")) {
            validate10.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.backends !== void 0) {
            let data0 = data.backends;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (!validate11(data0[i0], { instancePath: instancePath + "/backends/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate11.errors : vErrors.concat(validate11.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate10.errors = [{ instancePath: instancePath + "/backends", schemaPath: "#/properties/backends/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate10.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate10.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate10.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate10.errors = vErrors;
  return errors === 0;
}
var validateCatalogSearchOutcome = validate13;
var schema15 = { "additionalProperties": false, "properties": { "artifact": { "$ref": "#/definitions/CatalogArtifactState" }, "dependencyCount": { "maximum": 512, "minimum": 0, "type": "integer" }, "displayDate": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "displayName": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "format": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "id": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "integrity": { "$ref": "#/definitions/CatalogIntegrityState" }, "modelDir": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "modelType": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "quantization": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "relatedAvailable": { "type": "boolean" }, "sizeBytes": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" } }, "pumasCatalogRow": true, "required": ["id", "modelDir", "displayName", "modelType", "dependencyCount", "relatedAvailable", "artifact", "integrity"], "type": "object" };
var func2 = Object.prototype.hasOwnProperty;
var func4 = require_ucs2length().default;
var schema17 = { "enum": ["part_file_present", "expected_files_missing"], "type": "string" };
var func0 = require_equal().default;
var pattern0 = new RegExp("^v1:[0-9a-f]{64}$", "u");
var pattern1 = new RegExp("^(?!.*(?:--|\\.\\.))(?!.*\\.[gG][iI][tT]$)[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?/[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?$", "u");
var pattern2 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern3 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern4 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate15(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
                          const err14 = { instancePath: instancePath + "/reasons/" + i0, schemaPath: "#/definitions/CatalogPartialReason/enum", keyword: "enum", params: { allowedValues: schema17.enum }, message: "must be equal to one of the allowed values" };
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
    validate15.errors = vErrors;
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
  validate15.errors = vErrors;
  return errors === 0;
}
var pattern5 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern6 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern7 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern8 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern9 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern10 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern11 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate14(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.id === void 0 && (missing0 = "id") || data.modelDir === void 0 && (missing0 = "modelDir") || data.displayName === void 0 && (missing0 = "displayName") || data.modelType === void 0 && (missing0 = "modelType") || data.dependencyCount === void 0 && (missing0 = "dependencyCount") || data.relatedAvailable === void 0 && (missing0 = "relatedAvailable") || data.artifact === void 0 && (missing0 = "artifact") || data.integrity === void 0 && (missing0 = "integrity")) {
        validate14.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema15.properties, key0)) {
            validate14.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.artifact !== void 0) {
            const _errs2 = errors;
            if (!validate15(data.artifact, { instancePath: instancePath + "/artifact", parentData: data, parentDataProperty: "artifact", rootData })) {
              vErrors = vErrors === null ? validate15.errors : vErrors.concat(validate15.errors);
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
                validate14.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs3) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 512 || isNaN(data1)) {
                    validate14.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 512 }, message: "must be <= 512" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate14.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate14.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                      return false;
                    } else {
                      if (data2.length === 0 || pattern5.test(data2)) {
                        validate14.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                        return false;
                      } else {
                        if (encodeURIComponent(data2).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                          validate14.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate14.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                        validate14.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                        return false;
                      } else {
                        if (data3.length === 0 || pattern6.test(data3)) {
                          validate14.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data3).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate14.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate14.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                          validate14.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                          return false;
                        } else {
                          if (data4.length === 0 || pattern7.test(data4)) {
                            validate14.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                            return false;
                          } else {
                            if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                              validate14.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate14.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                            validate14.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                            return false;
                          } else {
                            if (data5.length === 0 || pattern8.test(data5)) {
                              validate14.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                              return false;
                            } else {
                              if (encodeURIComponent(data5).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                validate14.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate14.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                          validate14.errors = vErrors;
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
                                validate14.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                return false;
                              } else {
                                if (data12.length === 0 || pattern9.test(data12)) {
                                  validate14.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                  return false;
                                } else {
                                  if (encodeURIComponent(data12).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                    validate14.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                    return false;
                                  }
                                }
                              }
                            } else {
                              validate14.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                  validate14.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                  return false;
                                } else {
                                  if (data13.length === 0 || pattern10.test(data13)) {
                                    validate14.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                    return false;
                                  } else {
                                    if (encodeURIComponent(data13).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                      validate14.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                      return false;
                                    }
                                  }
                                }
                              } else {
                                validate14.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                    validate14.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                    return false;
                                  } else {
                                    if (data14.length === 0 || pattern11.test(data14)) {
                                      validate14.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                      return false;
                                    } else {
                                      if (encodeURIComponent(data14).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        validate14.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                        return false;
                                      }
                                    }
                                  }
                                } else {
                                  validate14.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                  validate14.errors = [{ instancePath: instancePath + "/relatedAvailable", schemaPath: "#/properties/relatedAvailable/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
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
                                    validate14.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                    return false;
                                  }
                                  if (errors === _errs40) {
                                    if (typeof data16 == "number" && isFinite(data16)) {
                                      if (data16 > 9007199254740991 || isNaN(data16)) {
                                        validate14.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                        return false;
                                      } else {
                                        if (data16 < 0 || isNaN(data16)) {
                                          validate14.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate14.errors = [{ instancePath, schemaPath: "#/pumasCatalogRow", keyword: "pumasCatalogRow", params: {}, message: 'must pass "pumasCatalogRow" keyword validation' }];
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
      validate14.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate14.errors = vErrors;
  return errors === 0;
}
function validate13(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.models === void 0 && (missing0 = "models") || data.total_count === void 0 && (missing0 = "total_count") || data.query_time_ms === void 0 && (missing0 = "query_time_ms") || data.query === void 0 && (missing0 = "query")) {
        validate13.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "models" || key0 === "query" || key0 === "query_time_ms" || key0 === "success" || key0 === "total_count")) {
            validate13.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate14(data0[i0], { instancePath: instancePath + "/models/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate14.errors : vErrors.concat(validate14.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate13.errors = [{ instancePath: instancePath + "/models", schemaPath: "#/properties/models/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                    validate13.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                } else {
                  validate13.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                      validate13.errors = [{ instancePath: instancePath + "/query_time_ms", schemaPath: "#/properties/query_time_ms/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                      return false;
                    } else {
                      if (data3 < 0 || isNaN(data3)) {
                        validate13.errors = [{ instancePath: instancePath + "/query_time_ms", schemaPath: "#/properties/query_time_ms/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                        return false;
                      }
                    }
                  } else {
                    validate13.errors = [{ instancePath: instancePath + "/query_time_ms", schemaPath: "#/properties/query_time_ms/type", keyword: "type", params: { type: "number" }, message: "must be number" }];
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
                    validate13.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                    return false;
                  }
                  if (true !== data4) {
                    validate13.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
                      validate13.errors = [{ instancePath: instancePath + "/total_count", schemaPath: "#/properties/total_count/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                      return false;
                    }
                    if (errors === _errs11) {
                      if (typeof data5 == "number" && isFinite(data5)) {
                        if (data5 > 9007199254740991 || isNaN(data5)) {
                          validate13.errors = [{ instancePath: instancePath + "/total_count", schemaPath: "#/properties/total_count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                          return false;
                        } else {
                          if (data5 < 0 || isNaN(data5)) {
                            validate13.errors = [{ instancePath: instancePath + "/total_count", schemaPath: "#/properties/total_count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate13.errors = [{ instancePath, schemaPath: "#/pumasCatalogSearch", keyword: "pumasCatalogSearch", params: {}, message: 'must pass "pumasCatalogSearch" keyword validation' }];
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
      validate13.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate13.errors = vErrors;
  return errors === 0;
}
var validateConversionCancelledOutcome = validate18;
function validate18(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.cancelled === void 0 && (missing0 = "cancelled")) {
        validate18.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "cancelled" || key0 === "success")) {
            validate18.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.cancelled !== void 0) {
            const _errs2 = errors;
            if (typeof data.cancelled !== "boolean") {
              validate18.errors = [{ instancePath: instancePath + "/cancelled", schemaPath: "#/properties/cancelled/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
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
                validate18.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate18.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate18.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate18.errors = vErrors;
  return errors === 0;
}
var validateConversionEnvironmentOutcome = validate19;
function validate19(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.ready === void 0 && (missing0 = "ready")) {
        validate19.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "ready" || key0 === "success")) {
            validate19.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.ready !== void 0) {
            const _errs2 = errors;
            if (typeof data.ready !== "boolean") {
              validate19.errors = [{ instancePath: instancePath + "/ready", schemaPath: "#/properties/ready/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
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
                validate19.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate19.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate19.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate19.errors = vErrors;
  return errors === 0;
}
var validateConversionListOutcome = validate20;
var schema23 = { "additionalProperties": false, "properties": { "bytesWritten": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "conversionId": { "type": "string" }, "currentTensor": { "type": ["string", "null"] }, "direction": { "$ref": "#/definitions/ConversionDirection" }, "error": { "enum": [null, "The model conversion did not complete successfully."], "type": ["string", "null"] }, "estimatedOutputSize": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "outputModelId": { "type": ["string", "null"] }, "pipelineStep": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "pipelineStepLabel": { "type": ["string", "null"] }, "pipelineStepsTotal": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "sourceModelId": { "type": "string" }, "status": { "$ref": "#/definitions/ConversionStatus" }, "targetQuant": { "type": ["string", "null"] }, "tensorsCompleted": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "tensorsTotal": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] } }, "required": ["conversionId", "sourceModelId", "direction", "status", "progress", "currentTensor", "tensorsCompleted", "tensorsTotal", "bytesWritten", "estimatedOutputSize", "targetQuant", "error", "outputModelId", "pipelineStep", "pipelineStepsTotal", "pipelineStepLabel"], "type": "object" };
function validate21(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.conversionId === void 0 && (missing0 = "conversionId") || data.sourceModelId === void 0 && (missing0 = "sourceModelId") || data.direction === void 0 && (missing0 = "direction") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.currentTensor === void 0 && (missing0 = "currentTensor") || data.tensorsCompleted === void 0 && (missing0 = "tensorsCompleted") || data.tensorsTotal === void 0 && (missing0 = "tensorsTotal") || data.bytesWritten === void 0 && (missing0 = "bytesWritten") || data.estimatedOutputSize === void 0 && (missing0 = "estimatedOutputSize") || data.targetQuant === void 0 && (missing0 = "targetQuant") || data.error === void 0 && (missing0 = "error") || data.outputModelId === void 0 && (missing0 = "outputModelId") || data.pipelineStep === void 0 && (missing0 = "pipelineStep") || data.pipelineStepsTotal === void 0 && (missing0 = "pipelineStepsTotal") || data.pipelineStepLabel === void 0 && (missing0 = "pipelineStepLabel")) {
        validate21.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema23.properties, key0)) {
            validate21.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.bytesWritten !== void 0) {
            let data0 = data.bytesWritten;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0)) && data0 !== null) {
              validate21.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/type", keyword: "type", params: { type: schema23.properties.bytesWritten.type }, message: "must be integer,null" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 9007199254740991 || isNaN(data0)) {
                  validate21.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                  return false;
                } else {
                  if (data0 < 0 || isNaN(data0)) {
                    validate21.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
            if (data.conversionId !== void 0) {
              const _errs4 = errors;
              if (typeof data.conversionId !== "string") {
                validate21.errors = [{ instancePath: instancePath + "/conversionId", schemaPath: "#/properties/conversionId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.currentTensor !== void 0) {
                let data2 = data.currentTensor;
                const _errs6 = errors;
                if (typeof data2 !== "string" && data2 !== null) {
                  validate21.errors = [{ instancePath: instancePath + "/currentTensor", schemaPath: "#/properties/currentTensor/type", keyword: "type", params: { type: schema23.properties.currentTensor.type }, message: "must be string,null" }];
                  return false;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.direction !== void 0) {
                  let data3 = data.direction;
                  const _errs8 = errors;
                  const _errs10 = errors;
                  let valid2 = false;
                  let passing0 = null;
                  const _errs11 = errors;
                  if (typeof data3 !== "string") {
                    const err0 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err0];
                    } else {
                      vErrors.push(err0);
                    }
                    errors++;
                  }
                  if ("gguf_to_safetensors" !== data3) {
                    const err1 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/0/const", keyword: "const", params: { allowedValue: "gguf_to_safetensors" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err1];
                    } else {
                      vErrors.push(err1);
                    }
                    errors++;
                  }
                  var _valid0 = _errs11 === errors;
                  if (_valid0) {
                    valid2 = true;
                    passing0 = 0;
                  }
                  const _errs13 = errors;
                  if (typeof data3 !== "string") {
                    const err2 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err2];
                    } else {
                      vErrors.push(err2);
                    }
                    errors++;
                  }
                  if ("safetensors_to_gguf" !== data3) {
                    const err3 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/1/const", keyword: "const", params: { allowedValue: "safetensors_to_gguf" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err3];
                    } else {
                      vErrors.push(err3);
                    }
                    errors++;
                  }
                  var _valid0 = _errs13 === errors;
                  if (_valid0 && valid2) {
                    valid2 = false;
                    passing0 = [passing0, 1];
                  } else {
                    if (_valid0) {
                      valid2 = true;
                      passing0 = 1;
                    }
                    const _errs15 = errors;
                    if (typeof data3 !== "string") {
                      const err4 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err4];
                      } else {
                        vErrors.push(err4);
                      }
                      errors++;
                    }
                    if ("safetensors_to_quantized_gguf" !== data3) {
                      const err5 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/2/const", keyword: "const", params: { allowedValue: "safetensors_to_quantized_gguf" }, message: "must be equal to constant" };
                      if (vErrors === null) {
                        vErrors = [err5];
                      } else {
                        vErrors.push(err5);
                      }
                      errors++;
                    }
                    var _valid0 = _errs15 === errors;
                    if (_valid0 && valid2) {
                      valid2 = false;
                      passing0 = [passing0, 2];
                    } else {
                      if (_valid0) {
                        valid2 = true;
                        passing0 = 2;
                      }
                      const _errs17 = errors;
                      if (typeof data3 !== "string") {
                        const err6 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                        if (vErrors === null) {
                          vErrors = [err6];
                        } else {
                          vErrors.push(err6);
                        }
                        errors++;
                      }
                      if ("gguf_to_quantized_gguf" !== data3) {
                        const err7 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/3/const", keyword: "const", params: { allowedValue: "gguf_to_quantized_gguf" }, message: "must be equal to constant" };
                        if (vErrors === null) {
                          vErrors = [err7];
                        } else {
                          vErrors.push(err7);
                        }
                        errors++;
                      }
                      var _valid0 = _errs17 === errors;
                      if (_valid0 && valid2) {
                        valid2 = false;
                        passing0 = [passing0, 3];
                      } else {
                        if (_valid0) {
                          valid2 = true;
                          passing0 = 3;
                        }
                        const _errs19 = errors;
                        if (typeof data3 !== "string") {
                          const err8 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                          if (vErrors === null) {
                            vErrors = [err8];
                          } else {
                            vErrors.push(err8);
                          }
                          errors++;
                        }
                        if ("safetensors_to_nvfp4" !== data3) {
                          const err9 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/4/const", keyword: "const", params: { allowedValue: "safetensors_to_nvfp4" }, message: "must be equal to constant" };
                          if (vErrors === null) {
                            vErrors = [err9];
                          } else {
                            vErrors.push(err9);
                          }
                          errors++;
                        }
                        var _valid0 = _errs19 === errors;
                        if (_valid0 && valid2) {
                          valid2 = false;
                          passing0 = [passing0, 4];
                        } else {
                          if (_valid0) {
                            valid2 = true;
                            passing0 = 4;
                          }
                          const _errs21 = errors;
                          if (typeof data3 !== "string") {
                            const err10 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/5/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                            if (vErrors === null) {
                              vErrors = [err10];
                            } else {
                              vErrors.push(err10);
                            }
                            errors++;
                          }
                          if ("safetensors_to_sherry_qat" !== data3) {
                            const err11 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/5/const", keyword: "const", params: { allowedValue: "safetensors_to_sherry_qat" }, message: "must be equal to constant" };
                            if (vErrors === null) {
                              vErrors = [err11];
                            } else {
                              vErrors.push(err11);
                            }
                            errors++;
                          }
                          var _valid0 = _errs21 === errors;
                          if (_valid0 && valid2) {
                            valid2 = false;
                            passing0 = [passing0, 5];
                          } else {
                            if (_valid0) {
                              valid2 = true;
                              passing0 = 5;
                            }
                          }
                        }
                      }
                    }
                  }
                  if (!valid2) {
                    const err12 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
                    if (vErrors === null) {
                      vErrors = [err12];
                    } else {
                      vErrors.push(err12);
                    }
                    errors++;
                    validate21.errors = vErrors;
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
                  var valid0 = _errs8 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.error !== void 0) {
                    let data4 = data.error;
                    const _errs23 = errors;
                    if (typeof data4 !== "string" && data4 !== null) {
                      validate21.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema23.properties.error.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (!(data4 === null || data4 === "The model conversion did not complete successfully.")) {
                      validate21.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/enum", keyword: "enum", params: { allowedValues: schema23.properties.error.enum }, message: "must be equal to one of the allowed values" }];
                      return false;
                    }
                    var valid0 = _errs23 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.estimatedOutputSize !== void 0) {
                      let data5 = data.estimatedOutputSize;
                      const _errs25 = errors;
                      if (!(typeof data5 == "number" && (!(data5 % 1) && !isNaN(data5)) && isFinite(data5)) && data5 !== null) {
                        validate21.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/type", keyword: "type", params: { type: schema23.properties.estimatedOutputSize.type }, message: "must be integer,null" }];
                        return false;
                      }
                      if (errors === _errs25) {
                        if (typeof data5 == "number" && isFinite(data5)) {
                          if (data5 > 9007199254740991 || isNaN(data5)) {
                            validate21.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                            return false;
                          } else {
                            if (data5 < 0 || isNaN(data5)) {
                              validate21.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                              return false;
                            }
                          }
                        }
                      }
                      var valid0 = _errs25 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.outputModelId !== void 0) {
                        let data6 = data.outputModelId;
                        const _errs27 = errors;
                        if (typeof data6 !== "string" && data6 !== null) {
                          validate21.errors = [{ instancePath: instancePath + "/outputModelId", schemaPath: "#/properties/outputModelId/type", keyword: "type", params: { type: schema23.properties.outputModelId.type }, message: "must be string,null" }];
                          return false;
                        }
                        var valid0 = _errs27 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.pipelineStep !== void 0) {
                          let data7 = data.pipelineStep;
                          const _errs29 = errors;
                          if (!(typeof data7 == "number" && (!(data7 % 1) && !isNaN(data7)) && isFinite(data7)) && data7 !== null) {
                            validate21.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/type", keyword: "type", params: { type: schema23.properties.pipelineStep.type }, message: "must be integer,null" }];
                            return false;
                          }
                          if (errors === _errs29) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 4294967295 || isNaN(data7)) {
                                validate21.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate21.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                  return false;
                                }
                              }
                            }
                          }
                          var valid0 = _errs29 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.pipelineStepLabel !== void 0) {
                            let data8 = data.pipelineStepLabel;
                            const _errs31 = errors;
                            if (typeof data8 !== "string" && data8 !== null) {
                              validate21.errors = [{ instancePath: instancePath + "/pipelineStepLabel", schemaPath: "#/properties/pipelineStepLabel/type", keyword: "type", params: { type: schema23.properties.pipelineStepLabel.type }, message: "must be string,null" }];
                              return false;
                            }
                            var valid0 = _errs31 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.pipelineStepsTotal !== void 0) {
                              let data9 = data.pipelineStepsTotal;
                              const _errs33 = errors;
                              if (!(typeof data9 == "number" && (!(data9 % 1) && !isNaN(data9)) && isFinite(data9)) && data9 !== null) {
                                validate21.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/type", keyword: "type", params: { type: schema23.properties.pipelineStepsTotal.type }, message: "must be integer,null" }];
                                return false;
                              }
                              if (errors === _errs33) {
                                if (typeof data9 == "number" && isFinite(data9)) {
                                  if (data9 > 4294967295 || isNaN(data9)) {
                                    validate21.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                    return false;
                                  } else {
                                    if (data9 < 0 || isNaN(data9)) {
                                      validate21.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                      return false;
                                    }
                                  }
                                }
                              }
                              var valid0 = _errs33 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.progress !== void 0) {
                                let data10 = data.progress;
                                const _errs35 = errors;
                                if (!(typeof data10 == "number" && isFinite(data10)) && data10 !== null) {
                                  validate21.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema23.properties.progress.type }, message: "must be number,null" }];
                                  return false;
                                }
                                if (errors === _errs35) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 1 || isNaN(data10)) {
                                      validate21.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate21.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                        return false;
                                      }
                                    }
                                  }
                                }
                                var valid0 = _errs35 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.sourceModelId !== void 0) {
                                  const _errs37 = errors;
                                  if (typeof data.sourceModelId !== "string") {
                                    validate21.errors = [{ instancePath: instancePath + "/sourceModelId", schemaPath: "#/properties/sourceModelId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                    return false;
                                  }
                                  var valid0 = _errs37 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.status !== void 0) {
                                    let data12 = data.status;
                                    const _errs39 = errors;
                                    const _errs41 = errors;
                                    let valid4 = false;
                                    let passing1 = null;
                                    const _errs42 = errors;
                                    if (typeof data12 !== "string") {
                                      const err13 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                      if (vErrors === null) {
                                        vErrors = [err13];
                                      } else {
                                        vErrors.push(err13);
                                      }
                                      errors++;
                                    }
                                    if ("setting_up" !== data12) {
                                      const err14 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/0/const", keyword: "const", params: { allowedValue: "setting_up" }, message: "must be equal to constant" };
                                      if (vErrors === null) {
                                        vErrors = [err14];
                                      } else {
                                        vErrors.push(err14);
                                      }
                                      errors++;
                                    }
                                    var _valid1 = _errs42 === errors;
                                    if (_valid1) {
                                      valid4 = true;
                                      passing1 = 0;
                                    }
                                    const _errs44 = errors;
                                    if (typeof data12 !== "string") {
                                      const err15 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                      if (vErrors === null) {
                                        vErrors = [err15];
                                      } else {
                                        vErrors.push(err15);
                                      }
                                      errors++;
                                    }
                                    if ("validating" !== data12) {
                                      const err16 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/1/const", keyword: "const", params: { allowedValue: "validating" }, message: "must be equal to constant" };
                                      if (vErrors === null) {
                                        vErrors = [err16];
                                      } else {
                                        vErrors.push(err16);
                                      }
                                      errors++;
                                    }
                                    var _valid1 = _errs44 === errors;
                                    if (_valid1 && valid4) {
                                      valid4 = false;
                                      passing1 = [passing1, 1];
                                    } else {
                                      if (_valid1) {
                                        valid4 = true;
                                        passing1 = 1;
                                      }
                                      const _errs46 = errors;
                                      if (typeof data12 !== "string") {
                                        const err17 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                        if (vErrors === null) {
                                          vErrors = [err17];
                                        } else {
                                          vErrors.push(err17);
                                        }
                                        errors++;
                                      }
                                      if ("converting" !== data12) {
                                        const err18 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/2/const", keyword: "const", params: { allowedValue: "converting" }, message: "must be equal to constant" };
                                        if (vErrors === null) {
                                          vErrors = [err18];
                                        } else {
                                          vErrors.push(err18);
                                        }
                                        errors++;
                                      }
                                      var _valid1 = _errs46 === errors;
                                      if (_valid1 && valid4) {
                                        valid4 = false;
                                        passing1 = [passing1, 2];
                                      } else {
                                        if (_valid1) {
                                          valid4 = true;
                                          passing1 = 2;
                                        }
                                        const _errs48 = errors;
                                        if (typeof data12 !== "string") {
                                          const err19 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                          if (vErrors === null) {
                                            vErrors = [err19];
                                          } else {
                                            vErrors.push(err19);
                                          }
                                          errors++;
                                        }
                                        if ("writing" !== data12) {
                                          const err20 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/3/const", keyword: "const", params: { allowedValue: "writing" }, message: "must be equal to constant" };
                                          if (vErrors === null) {
                                            vErrors = [err20];
                                          } else {
                                            vErrors.push(err20);
                                          }
                                          errors++;
                                        }
                                        var _valid1 = _errs48 === errors;
                                        if (_valid1 && valid4) {
                                          valid4 = false;
                                          passing1 = [passing1, 3];
                                        } else {
                                          if (_valid1) {
                                            valid4 = true;
                                            passing1 = 3;
                                          }
                                          const _errs50 = errors;
                                          if (typeof data12 !== "string") {
                                            const err21 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                            if (vErrors === null) {
                                              vErrors = [err21];
                                            } else {
                                              vErrors.push(err21);
                                            }
                                            errors++;
                                          }
                                          if ("importing" !== data12) {
                                            const err22 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/4/const", keyword: "const", params: { allowedValue: "importing" }, message: "must be equal to constant" };
                                            if (vErrors === null) {
                                              vErrors = [err22];
                                            } else {
                                              vErrors.push(err22);
                                            }
                                            errors++;
                                          }
                                          var _valid1 = _errs50 === errors;
                                          if (_valid1 && valid4) {
                                            valid4 = false;
                                            passing1 = [passing1, 4];
                                          } else {
                                            if (_valid1) {
                                              valid4 = true;
                                              passing1 = 4;
                                            }
                                            const _errs52 = errors;
                                            if (typeof data12 !== "string") {
                                              const err23 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/5/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                              if (vErrors === null) {
                                                vErrors = [err23];
                                              } else {
                                                vErrors.push(err23);
                                              }
                                              errors++;
                                            }
                                            if ("completed" !== data12) {
                                              const err24 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/5/const", keyword: "const", params: { allowedValue: "completed" }, message: "must be equal to constant" };
                                              if (vErrors === null) {
                                                vErrors = [err24];
                                              } else {
                                                vErrors.push(err24);
                                              }
                                              errors++;
                                            }
                                            var _valid1 = _errs52 === errors;
                                            if (_valid1 && valid4) {
                                              valid4 = false;
                                              passing1 = [passing1, 5];
                                            } else {
                                              if (_valid1) {
                                                valid4 = true;
                                                passing1 = 5;
                                              }
                                              const _errs54 = errors;
                                              if (typeof data12 !== "string") {
                                                const err25 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/6/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                if (vErrors === null) {
                                                  vErrors = [err25];
                                                } else {
                                                  vErrors.push(err25);
                                                }
                                                errors++;
                                              }
                                              if ("cancelled" !== data12) {
                                                const err26 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/6/const", keyword: "const", params: { allowedValue: "cancelled" }, message: "must be equal to constant" };
                                                if (vErrors === null) {
                                                  vErrors = [err26];
                                                } else {
                                                  vErrors.push(err26);
                                                }
                                                errors++;
                                              }
                                              var _valid1 = _errs54 === errors;
                                              if (_valid1 && valid4) {
                                                valid4 = false;
                                                passing1 = [passing1, 6];
                                              } else {
                                                if (_valid1) {
                                                  valid4 = true;
                                                  passing1 = 6;
                                                }
                                                const _errs56 = errors;
                                                if (typeof data12 !== "string") {
                                                  const err27 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/7/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                  if (vErrors === null) {
                                                    vErrors = [err27];
                                                  } else {
                                                    vErrors.push(err27);
                                                  }
                                                  errors++;
                                                }
                                                if ("error" !== data12) {
                                                  const err28 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/7/const", keyword: "const", params: { allowedValue: "error" }, message: "must be equal to constant" };
                                                  if (vErrors === null) {
                                                    vErrors = [err28];
                                                  } else {
                                                    vErrors.push(err28);
                                                  }
                                                  errors++;
                                                }
                                                var _valid1 = _errs56 === errors;
                                                if (_valid1 && valid4) {
                                                  valid4 = false;
                                                  passing1 = [passing1, 7];
                                                } else {
                                                  if (_valid1) {
                                                    valid4 = true;
                                                    passing1 = 7;
                                                  }
                                                  const _errs58 = errors;
                                                  if (typeof data12 !== "string") {
                                                    const err29 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/8/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                    if (vErrors === null) {
                                                      vErrors = [err29];
                                                    } else {
                                                      vErrors.push(err29);
                                                    }
                                                    errors++;
                                                  }
                                                  if ("building_toolchain" !== data12) {
                                                    const err30 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/8/const", keyword: "const", params: { allowedValue: "building_toolchain" }, message: "must be equal to constant" };
                                                    if (vErrors === null) {
                                                      vErrors = [err30];
                                                    } else {
                                                      vErrors.push(err30);
                                                    }
                                                    errors++;
                                                  }
                                                  var _valid1 = _errs58 === errors;
                                                  if (_valid1 && valid4) {
                                                    valid4 = false;
                                                    passing1 = [passing1, 8];
                                                  } else {
                                                    if (_valid1) {
                                                      valid4 = true;
                                                      passing1 = 8;
                                                    }
                                                    const _errs60 = errors;
                                                    if (typeof data12 !== "string") {
                                                      const err31 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/9/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                      if (vErrors === null) {
                                                        vErrors = [err31];
                                                      } else {
                                                        vErrors.push(err31);
                                                      }
                                                      errors++;
                                                    }
                                                    if ("generating_f16_gguf" !== data12) {
                                                      const err32 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/9/const", keyword: "const", params: { allowedValue: "generating_f16_gguf" }, message: "must be equal to constant" };
                                                      if (vErrors === null) {
                                                        vErrors = [err32];
                                                      } else {
                                                        vErrors.push(err32);
                                                      }
                                                      errors++;
                                                    }
                                                    var _valid1 = _errs60 === errors;
                                                    if (_valid1 && valid4) {
                                                      valid4 = false;
                                                      passing1 = [passing1, 9];
                                                    } else {
                                                      if (_valid1) {
                                                        valid4 = true;
                                                        passing1 = 9;
                                                      }
                                                      const _errs62 = errors;
                                                      if (typeof data12 !== "string") {
                                                        const err33 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/10/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                        if (vErrors === null) {
                                                          vErrors = [err33];
                                                        } else {
                                                          vErrors.push(err33);
                                                        }
                                                        errors++;
                                                      }
                                                      if ("computing_imatrix" !== data12) {
                                                        const err34 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/10/const", keyword: "const", params: { allowedValue: "computing_imatrix" }, message: "must be equal to constant" };
                                                        if (vErrors === null) {
                                                          vErrors = [err34];
                                                        } else {
                                                          vErrors.push(err34);
                                                        }
                                                        errors++;
                                                      }
                                                      var _valid1 = _errs62 === errors;
                                                      if (_valid1 && valid4) {
                                                        valid4 = false;
                                                        passing1 = [passing1, 10];
                                                      } else {
                                                        if (_valid1) {
                                                          valid4 = true;
                                                          passing1 = 10;
                                                        }
                                                        const _errs64 = errors;
                                                        if (typeof data12 !== "string") {
                                                          const err35 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/11/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                          if (vErrors === null) {
                                                            vErrors = [err35];
                                                          } else {
                                                            vErrors.push(err35);
                                                          }
                                                          errors++;
                                                        }
                                                        if ("quantizing" !== data12) {
                                                          const err36 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/11/const", keyword: "const", params: { allowedValue: "quantizing" }, message: "must be equal to constant" };
                                                          if (vErrors === null) {
                                                            vErrors = [err36];
                                                          } else {
                                                            vErrors.push(err36);
                                                          }
                                                          errors++;
                                                        }
                                                        var _valid1 = _errs64 === errors;
                                                        if (_valid1 && valid4) {
                                                          valid4 = false;
                                                          passing1 = [passing1, 11];
                                                        } else {
                                                          if (_valid1) {
                                                            valid4 = true;
                                                            passing1 = 11;
                                                          }
                                                          const _errs66 = errors;
                                                          if (typeof data12 !== "string") {
                                                            const err37 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/12/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                            if (vErrors === null) {
                                                              vErrors = [err37];
                                                            } else {
                                                              vErrors.push(err37);
                                                            }
                                                            errors++;
                                                          }
                                                          if ("calibrating" !== data12) {
                                                            const err38 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/12/const", keyword: "const", params: { allowedValue: "calibrating" }, message: "must be equal to constant" };
                                                            if (vErrors === null) {
                                                              vErrors = [err38];
                                                            } else {
                                                              vErrors.push(err38);
                                                            }
                                                            errors++;
                                                          }
                                                          var _valid1 = _errs66 === errors;
                                                          if (_valid1 && valid4) {
                                                            valid4 = false;
                                                            passing1 = [passing1, 12];
                                                          } else {
                                                            if (_valid1) {
                                                              valid4 = true;
                                                              passing1 = 12;
                                                            }
                                                            const _errs68 = errors;
                                                            if (typeof data12 !== "string") {
                                                              const err39 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/13/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                              if (vErrors === null) {
                                                                vErrors = [err39];
                                                              } else {
                                                                vErrors.push(err39);
                                                              }
                                                              errors++;
                                                            }
                                                            if ("training" !== data12) {
                                                              const err40 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/13/const", keyword: "const", params: { allowedValue: "training" }, message: "must be equal to constant" };
                                                              if (vErrors === null) {
                                                                vErrors = [err40];
                                                              } else {
                                                                vErrors.push(err40);
                                                              }
                                                              errors++;
                                                            }
                                                            var _valid1 = _errs68 === errors;
                                                            if (_valid1 && valid4) {
                                                              valid4 = false;
                                                              passing1 = [passing1, 13];
                                                            } else {
                                                              if (_valid1) {
                                                                valid4 = true;
                                                                passing1 = 13;
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
                                    if (!valid4) {
                                      const err41 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf", keyword: "oneOf", params: { passingSchemas: passing1 }, message: "must match exactly one schema in oneOf" };
                                      if (vErrors === null) {
                                        vErrors = [err41];
                                      } else {
                                        vErrors.push(err41);
                                      }
                                      errors++;
                                      validate21.errors = vErrors;
                                      return false;
                                    } else {
                                      errors = _errs41;
                                      if (vErrors !== null) {
                                        if (_errs41) {
                                          vErrors.length = _errs41;
                                        } else {
                                          vErrors = null;
                                        }
                                      }
                                    }
                                    var valid0 = _errs39 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.targetQuant !== void 0) {
                                      let data13 = data.targetQuant;
                                      const _errs70 = errors;
                                      if (typeof data13 !== "string" && data13 !== null) {
                                        validate21.errors = [{ instancePath: instancePath + "/targetQuant", schemaPath: "#/properties/targetQuant/type", keyword: "type", params: { type: schema23.properties.targetQuant.type }, message: "must be string,null" }];
                                        return false;
                                      }
                                      var valid0 = _errs70 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                    if (valid0) {
                                      if (data.tensorsCompleted !== void 0) {
                                        let data14 = data.tensorsCompleted;
                                        const _errs72 = errors;
                                        if (!(typeof data14 == "number" && (!(data14 % 1) && !isNaN(data14)) && isFinite(data14)) && data14 !== null) {
                                          validate21.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/type", keyword: "type", params: { type: schema23.properties.tensorsCompleted.type }, message: "must be integer,null" }];
                                          return false;
                                        }
                                        if (errors === _errs72) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 4294967295 || isNaN(data14)) {
                                              validate21.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate21.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                return false;
                                              }
                                            }
                                          }
                                        }
                                        var valid0 = _errs72 === errors;
                                      } else {
                                        var valid0 = true;
                                      }
                                      if (valid0) {
                                        if (data.tensorsTotal !== void 0) {
                                          let data15 = data.tensorsTotal;
                                          const _errs74 = errors;
                                          if (!(typeof data15 == "number" && (!(data15 % 1) && !isNaN(data15)) && isFinite(data15)) && data15 !== null) {
                                            validate21.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/type", keyword: "type", params: { type: schema23.properties.tensorsTotal.type }, message: "must be integer,null" }];
                                            return false;
                                          }
                                          if (errors === _errs74) {
                                            if (typeof data15 == "number" && isFinite(data15)) {
                                              if (data15 > 4294967295 || isNaN(data15)) {
                                                validate21.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                                return false;
                                              } else {
                                                if (data15 < 0 || isNaN(data15)) {
                                                  validate21.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                  return false;
                                                }
                                              }
                                            }
                                          }
                                          var valid0 = _errs74 === errors;
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
    } else {
      validate21.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate21.errors = vErrors;
  return errors === 0;
}
function validate20(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.conversions === void 0 && (missing0 = "conversions")) {
        validate20.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "conversions" || key0 === "success")) {
            validate20.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.conversions !== void 0) {
            let data0 = data.conversions;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (!validate21(data0[i0], { instancePath: instancePath + "/conversions/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate21.errors : vErrors.concat(validate21.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate20.errors = [{ instancePath: instancePath + "/conversions", schemaPath: "#/properties/conversions/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate20.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate20.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate20.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate20.errors = vErrors;
  return errors === 0;
}
var validateConversionProgressResponse = validate23;
var schema27 = { "additionalProperties": false, "properties": { "bytesWritten": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "conversionId": { "type": "string" }, "currentTensor": { "type": ["string", "null"] }, "direction": { "$ref": "#/definitions/ConversionDirection" }, "error": { "enum": [null, "The model conversion did not complete successfully."], "type": ["string", "null"] }, "estimatedOutputSize": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "outputModelId": { "type": ["string", "null"] }, "pipelineStep": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "pipelineStepLabel": { "type": ["string", "null"] }, "pipelineStepsTotal": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "sourceModelId": { "type": "string" }, "status": { "$ref": "#/definitions/ConversionStatus" }, "targetQuant": { "type": ["string", "null"] }, "tensorsCompleted": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "tensorsTotal": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] } }, "required": ["conversionId", "sourceModelId", "direction", "status", "progress", "currentTensor", "tensorsCompleted", "tensorsTotal", "bytesWritten", "estimatedOutputSize", "targetQuant", "error", "outputModelId", "pipelineStep", "pipelineStepsTotal", "pipelineStepLabel"], "type": "object" };
function validate24(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.conversionId === void 0 && (missing0 = "conversionId") || data.sourceModelId === void 0 && (missing0 = "sourceModelId") || data.direction === void 0 && (missing0 = "direction") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.currentTensor === void 0 && (missing0 = "currentTensor") || data.tensorsCompleted === void 0 && (missing0 = "tensorsCompleted") || data.tensorsTotal === void 0 && (missing0 = "tensorsTotal") || data.bytesWritten === void 0 && (missing0 = "bytesWritten") || data.estimatedOutputSize === void 0 && (missing0 = "estimatedOutputSize") || data.targetQuant === void 0 && (missing0 = "targetQuant") || data.error === void 0 && (missing0 = "error") || data.outputModelId === void 0 && (missing0 = "outputModelId") || data.pipelineStep === void 0 && (missing0 = "pipelineStep") || data.pipelineStepsTotal === void 0 && (missing0 = "pipelineStepsTotal") || data.pipelineStepLabel === void 0 && (missing0 = "pipelineStepLabel")) {
        validate24.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema27.properties, key0)) {
            validate24.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.bytesWritten !== void 0) {
            let data0 = data.bytesWritten;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0)) && data0 !== null) {
              validate24.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/type", keyword: "type", params: { type: schema27.properties.bytesWritten.type }, message: "must be integer,null" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 9007199254740991 || isNaN(data0)) {
                  validate24.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                  return false;
                } else {
                  if (data0 < 0 || isNaN(data0)) {
                    validate24.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
            if (data.conversionId !== void 0) {
              const _errs4 = errors;
              if (typeof data.conversionId !== "string") {
                validate24.errors = [{ instancePath: instancePath + "/conversionId", schemaPath: "#/properties/conversionId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.currentTensor !== void 0) {
                let data2 = data.currentTensor;
                const _errs6 = errors;
                if (typeof data2 !== "string" && data2 !== null) {
                  validate24.errors = [{ instancePath: instancePath + "/currentTensor", schemaPath: "#/properties/currentTensor/type", keyword: "type", params: { type: schema27.properties.currentTensor.type }, message: "must be string,null" }];
                  return false;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.direction !== void 0) {
                  let data3 = data.direction;
                  const _errs8 = errors;
                  const _errs10 = errors;
                  let valid2 = false;
                  let passing0 = null;
                  const _errs11 = errors;
                  if (typeof data3 !== "string") {
                    const err0 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err0];
                    } else {
                      vErrors.push(err0);
                    }
                    errors++;
                  }
                  if ("gguf_to_safetensors" !== data3) {
                    const err1 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/0/const", keyword: "const", params: { allowedValue: "gguf_to_safetensors" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err1];
                    } else {
                      vErrors.push(err1);
                    }
                    errors++;
                  }
                  var _valid0 = _errs11 === errors;
                  if (_valid0) {
                    valid2 = true;
                    passing0 = 0;
                  }
                  const _errs13 = errors;
                  if (typeof data3 !== "string") {
                    const err2 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err2];
                    } else {
                      vErrors.push(err2);
                    }
                    errors++;
                  }
                  if ("safetensors_to_gguf" !== data3) {
                    const err3 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/1/const", keyword: "const", params: { allowedValue: "safetensors_to_gguf" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err3];
                    } else {
                      vErrors.push(err3);
                    }
                    errors++;
                  }
                  var _valid0 = _errs13 === errors;
                  if (_valid0 && valid2) {
                    valid2 = false;
                    passing0 = [passing0, 1];
                  } else {
                    if (_valid0) {
                      valid2 = true;
                      passing0 = 1;
                    }
                    const _errs15 = errors;
                    if (typeof data3 !== "string") {
                      const err4 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err4];
                      } else {
                        vErrors.push(err4);
                      }
                      errors++;
                    }
                    if ("safetensors_to_quantized_gguf" !== data3) {
                      const err5 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/2/const", keyword: "const", params: { allowedValue: "safetensors_to_quantized_gguf" }, message: "must be equal to constant" };
                      if (vErrors === null) {
                        vErrors = [err5];
                      } else {
                        vErrors.push(err5);
                      }
                      errors++;
                    }
                    var _valid0 = _errs15 === errors;
                    if (_valid0 && valid2) {
                      valid2 = false;
                      passing0 = [passing0, 2];
                    } else {
                      if (_valid0) {
                        valid2 = true;
                        passing0 = 2;
                      }
                      const _errs17 = errors;
                      if (typeof data3 !== "string") {
                        const err6 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                        if (vErrors === null) {
                          vErrors = [err6];
                        } else {
                          vErrors.push(err6);
                        }
                        errors++;
                      }
                      if ("gguf_to_quantized_gguf" !== data3) {
                        const err7 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/3/const", keyword: "const", params: { allowedValue: "gguf_to_quantized_gguf" }, message: "must be equal to constant" };
                        if (vErrors === null) {
                          vErrors = [err7];
                        } else {
                          vErrors.push(err7);
                        }
                        errors++;
                      }
                      var _valid0 = _errs17 === errors;
                      if (_valid0 && valid2) {
                        valid2 = false;
                        passing0 = [passing0, 3];
                      } else {
                        if (_valid0) {
                          valid2 = true;
                          passing0 = 3;
                        }
                        const _errs19 = errors;
                        if (typeof data3 !== "string") {
                          const err8 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                          if (vErrors === null) {
                            vErrors = [err8];
                          } else {
                            vErrors.push(err8);
                          }
                          errors++;
                        }
                        if ("safetensors_to_nvfp4" !== data3) {
                          const err9 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/4/const", keyword: "const", params: { allowedValue: "safetensors_to_nvfp4" }, message: "must be equal to constant" };
                          if (vErrors === null) {
                            vErrors = [err9];
                          } else {
                            vErrors.push(err9);
                          }
                          errors++;
                        }
                        var _valid0 = _errs19 === errors;
                        if (_valid0 && valid2) {
                          valid2 = false;
                          passing0 = [passing0, 4];
                        } else {
                          if (_valid0) {
                            valid2 = true;
                            passing0 = 4;
                          }
                          const _errs21 = errors;
                          if (typeof data3 !== "string") {
                            const err10 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/5/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                            if (vErrors === null) {
                              vErrors = [err10];
                            } else {
                              vErrors.push(err10);
                            }
                            errors++;
                          }
                          if ("safetensors_to_sherry_qat" !== data3) {
                            const err11 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf/5/const", keyword: "const", params: { allowedValue: "safetensors_to_sherry_qat" }, message: "must be equal to constant" };
                            if (vErrors === null) {
                              vErrors = [err11];
                            } else {
                              vErrors.push(err11);
                            }
                            errors++;
                          }
                          var _valid0 = _errs21 === errors;
                          if (_valid0 && valid2) {
                            valid2 = false;
                            passing0 = [passing0, 5];
                          } else {
                            if (_valid0) {
                              valid2 = true;
                              passing0 = 5;
                            }
                          }
                        }
                      }
                    }
                  }
                  if (!valid2) {
                    const err12 = { instancePath: instancePath + "/direction", schemaPath: "#/definitions/ConversionDirection/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
                    if (vErrors === null) {
                      vErrors = [err12];
                    } else {
                      vErrors.push(err12);
                    }
                    errors++;
                    validate24.errors = vErrors;
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
                  var valid0 = _errs8 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.error !== void 0) {
                    let data4 = data.error;
                    const _errs23 = errors;
                    if (typeof data4 !== "string" && data4 !== null) {
                      validate24.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema27.properties.error.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (!(data4 === null || data4 === "The model conversion did not complete successfully.")) {
                      validate24.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/enum", keyword: "enum", params: { allowedValues: schema27.properties.error.enum }, message: "must be equal to one of the allowed values" }];
                      return false;
                    }
                    var valid0 = _errs23 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.estimatedOutputSize !== void 0) {
                      let data5 = data.estimatedOutputSize;
                      const _errs25 = errors;
                      if (!(typeof data5 == "number" && (!(data5 % 1) && !isNaN(data5)) && isFinite(data5)) && data5 !== null) {
                        validate24.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/type", keyword: "type", params: { type: schema27.properties.estimatedOutputSize.type }, message: "must be integer,null" }];
                        return false;
                      }
                      if (errors === _errs25) {
                        if (typeof data5 == "number" && isFinite(data5)) {
                          if (data5 > 9007199254740991 || isNaN(data5)) {
                            validate24.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                            return false;
                          } else {
                            if (data5 < 0 || isNaN(data5)) {
                              validate24.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                              return false;
                            }
                          }
                        }
                      }
                      var valid0 = _errs25 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.outputModelId !== void 0) {
                        let data6 = data.outputModelId;
                        const _errs27 = errors;
                        if (typeof data6 !== "string" && data6 !== null) {
                          validate24.errors = [{ instancePath: instancePath + "/outputModelId", schemaPath: "#/properties/outputModelId/type", keyword: "type", params: { type: schema27.properties.outputModelId.type }, message: "must be string,null" }];
                          return false;
                        }
                        var valid0 = _errs27 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.pipelineStep !== void 0) {
                          let data7 = data.pipelineStep;
                          const _errs29 = errors;
                          if (!(typeof data7 == "number" && (!(data7 % 1) && !isNaN(data7)) && isFinite(data7)) && data7 !== null) {
                            validate24.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/type", keyword: "type", params: { type: schema27.properties.pipelineStep.type }, message: "must be integer,null" }];
                            return false;
                          }
                          if (errors === _errs29) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 4294967295 || isNaN(data7)) {
                                validate24.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate24.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                  return false;
                                }
                              }
                            }
                          }
                          var valid0 = _errs29 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.pipelineStepLabel !== void 0) {
                            let data8 = data.pipelineStepLabel;
                            const _errs31 = errors;
                            if (typeof data8 !== "string" && data8 !== null) {
                              validate24.errors = [{ instancePath: instancePath + "/pipelineStepLabel", schemaPath: "#/properties/pipelineStepLabel/type", keyword: "type", params: { type: schema27.properties.pipelineStepLabel.type }, message: "must be string,null" }];
                              return false;
                            }
                            var valid0 = _errs31 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.pipelineStepsTotal !== void 0) {
                              let data9 = data.pipelineStepsTotal;
                              const _errs33 = errors;
                              if (!(typeof data9 == "number" && (!(data9 % 1) && !isNaN(data9)) && isFinite(data9)) && data9 !== null) {
                                validate24.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/type", keyword: "type", params: { type: schema27.properties.pipelineStepsTotal.type }, message: "must be integer,null" }];
                                return false;
                              }
                              if (errors === _errs33) {
                                if (typeof data9 == "number" && isFinite(data9)) {
                                  if (data9 > 4294967295 || isNaN(data9)) {
                                    validate24.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                    return false;
                                  } else {
                                    if (data9 < 0 || isNaN(data9)) {
                                      validate24.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                      return false;
                                    }
                                  }
                                }
                              }
                              var valid0 = _errs33 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.progress !== void 0) {
                                let data10 = data.progress;
                                const _errs35 = errors;
                                if (!(typeof data10 == "number" && isFinite(data10)) && data10 !== null) {
                                  validate24.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema27.properties.progress.type }, message: "must be number,null" }];
                                  return false;
                                }
                                if (errors === _errs35) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 1 || isNaN(data10)) {
                                      validate24.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate24.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                        return false;
                                      }
                                    }
                                  }
                                }
                                var valid0 = _errs35 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.sourceModelId !== void 0) {
                                  const _errs37 = errors;
                                  if (typeof data.sourceModelId !== "string") {
                                    validate24.errors = [{ instancePath: instancePath + "/sourceModelId", schemaPath: "#/properties/sourceModelId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                    return false;
                                  }
                                  var valid0 = _errs37 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.status !== void 0) {
                                    let data12 = data.status;
                                    const _errs39 = errors;
                                    const _errs41 = errors;
                                    let valid4 = false;
                                    let passing1 = null;
                                    const _errs42 = errors;
                                    if (typeof data12 !== "string") {
                                      const err13 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                      if (vErrors === null) {
                                        vErrors = [err13];
                                      } else {
                                        vErrors.push(err13);
                                      }
                                      errors++;
                                    }
                                    if ("setting_up" !== data12) {
                                      const err14 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/0/const", keyword: "const", params: { allowedValue: "setting_up" }, message: "must be equal to constant" };
                                      if (vErrors === null) {
                                        vErrors = [err14];
                                      } else {
                                        vErrors.push(err14);
                                      }
                                      errors++;
                                    }
                                    var _valid1 = _errs42 === errors;
                                    if (_valid1) {
                                      valid4 = true;
                                      passing1 = 0;
                                    }
                                    const _errs44 = errors;
                                    if (typeof data12 !== "string") {
                                      const err15 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                      if (vErrors === null) {
                                        vErrors = [err15];
                                      } else {
                                        vErrors.push(err15);
                                      }
                                      errors++;
                                    }
                                    if ("validating" !== data12) {
                                      const err16 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/1/const", keyword: "const", params: { allowedValue: "validating" }, message: "must be equal to constant" };
                                      if (vErrors === null) {
                                        vErrors = [err16];
                                      } else {
                                        vErrors.push(err16);
                                      }
                                      errors++;
                                    }
                                    var _valid1 = _errs44 === errors;
                                    if (_valid1 && valid4) {
                                      valid4 = false;
                                      passing1 = [passing1, 1];
                                    } else {
                                      if (_valid1) {
                                        valid4 = true;
                                        passing1 = 1;
                                      }
                                      const _errs46 = errors;
                                      if (typeof data12 !== "string") {
                                        const err17 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                        if (vErrors === null) {
                                          vErrors = [err17];
                                        } else {
                                          vErrors.push(err17);
                                        }
                                        errors++;
                                      }
                                      if ("converting" !== data12) {
                                        const err18 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/2/const", keyword: "const", params: { allowedValue: "converting" }, message: "must be equal to constant" };
                                        if (vErrors === null) {
                                          vErrors = [err18];
                                        } else {
                                          vErrors.push(err18);
                                        }
                                        errors++;
                                      }
                                      var _valid1 = _errs46 === errors;
                                      if (_valid1 && valid4) {
                                        valid4 = false;
                                        passing1 = [passing1, 2];
                                      } else {
                                        if (_valid1) {
                                          valid4 = true;
                                          passing1 = 2;
                                        }
                                        const _errs48 = errors;
                                        if (typeof data12 !== "string") {
                                          const err19 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                          if (vErrors === null) {
                                            vErrors = [err19];
                                          } else {
                                            vErrors.push(err19);
                                          }
                                          errors++;
                                        }
                                        if ("writing" !== data12) {
                                          const err20 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/3/const", keyword: "const", params: { allowedValue: "writing" }, message: "must be equal to constant" };
                                          if (vErrors === null) {
                                            vErrors = [err20];
                                          } else {
                                            vErrors.push(err20);
                                          }
                                          errors++;
                                        }
                                        var _valid1 = _errs48 === errors;
                                        if (_valid1 && valid4) {
                                          valid4 = false;
                                          passing1 = [passing1, 3];
                                        } else {
                                          if (_valid1) {
                                            valid4 = true;
                                            passing1 = 3;
                                          }
                                          const _errs50 = errors;
                                          if (typeof data12 !== "string") {
                                            const err21 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                            if (vErrors === null) {
                                              vErrors = [err21];
                                            } else {
                                              vErrors.push(err21);
                                            }
                                            errors++;
                                          }
                                          if ("importing" !== data12) {
                                            const err22 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/4/const", keyword: "const", params: { allowedValue: "importing" }, message: "must be equal to constant" };
                                            if (vErrors === null) {
                                              vErrors = [err22];
                                            } else {
                                              vErrors.push(err22);
                                            }
                                            errors++;
                                          }
                                          var _valid1 = _errs50 === errors;
                                          if (_valid1 && valid4) {
                                            valid4 = false;
                                            passing1 = [passing1, 4];
                                          } else {
                                            if (_valid1) {
                                              valid4 = true;
                                              passing1 = 4;
                                            }
                                            const _errs52 = errors;
                                            if (typeof data12 !== "string") {
                                              const err23 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/5/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                              if (vErrors === null) {
                                                vErrors = [err23];
                                              } else {
                                                vErrors.push(err23);
                                              }
                                              errors++;
                                            }
                                            if ("completed" !== data12) {
                                              const err24 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/5/const", keyword: "const", params: { allowedValue: "completed" }, message: "must be equal to constant" };
                                              if (vErrors === null) {
                                                vErrors = [err24];
                                              } else {
                                                vErrors.push(err24);
                                              }
                                              errors++;
                                            }
                                            var _valid1 = _errs52 === errors;
                                            if (_valid1 && valid4) {
                                              valid4 = false;
                                              passing1 = [passing1, 5];
                                            } else {
                                              if (_valid1) {
                                                valid4 = true;
                                                passing1 = 5;
                                              }
                                              const _errs54 = errors;
                                              if (typeof data12 !== "string") {
                                                const err25 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/6/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                if (vErrors === null) {
                                                  vErrors = [err25];
                                                } else {
                                                  vErrors.push(err25);
                                                }
                                                errors++;
                                              }
                                              if ("cancelled" !== data12) {
                                                const err26 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/6/const", keyword: "const", params: { allowedValue: "cancelled" }, message: "must be equal to constant" };
                                                if (vErrors === null) {
                                                  vErrors = [err26];
                                                } else {
                                                  vErrors.push(err26);
                                                }
                                                errors++;
                                              }
                                              var _valid1 = _errs54 === errors;
                                              if (_valid1 && valid4) {
                                                valid4 = false;
                                                passing1 = [passing1, 6];
                                              } else {
                                                if (_valid1) {
                                                  valid4 = true;
                                                  passing1 = 6;
                                                }
                                                const _errs56 = errors;
                                                if (typeof data12 !== "string") {
                                                  const err27 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/7/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                  if (vErrors === null) {
                                                    vErrors = [err27];
                                                  } else {
                                                    vErrors.push(err27);
                                                  }
                                                  errors++;
                                                }
                                                if ("error" !== data12) {
                                                  const err28 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/7/const", keyword: "const", params: { allowedValue: "error" }, message: "must be equal to constant" };
                                                  if (vErrors === null) {
                                                    vErrors = [err28];
                                                  } else {
                                                    vErrors.push(err28);
                                                  }
                                                  errors++;
                                                }
                                                var _valid1 = _errs56 === errors;
                                                if (_valid1 && valid4) {
                                                  valid4 = false;
                                                  passing1 = [passing1, 7];
                                                } else {
                                                  if (_valid1) {
                                                    valid4 = true;
                                                    passing1 = 7;
                                                  }
                                                  const _errs58 = errors;
                                                  if (typeof data12 !== "string") {
                                                    const err29 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/8/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                    if (vErrors === null) {
                                                      vErrors = [err29];
                                                    } else {
                                                      vErrors.push(err29);
                                                    }
                                                    errors++;
                                                  }
                                                  if ("building_toolchain" !== data12) {
                                                    const err30 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/8/const", keyword: "const", params: { allowedValue: "building_toolchain" }, message: "must be equal to constant" };
                                                    if (vErrors === null) {
                                                      vErrors = [err30];
                                                    } else {
                                                      vErrors.push(err30);
                                                    }
                                                    errors++;
                                                  }
                                                  var _valid1 = _errs58 === errors;
                                                  if (_valid1 && valid4) {
                                                    valid4 = false;
                                                    passing1 = [passing1, 8];
                                                  } else {
                                                    if (_valid1) {
                                                      valid4 = true;
                                                      passing1 = 8;
                                                    }
                                                    const _errs60 = errors;
                                                    if (typeof data12 !== "string") {
                                                      const err31 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/9/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                      if (vErrors === null) {
                                                        vErrors = [err31];
                                                      } else {
                                                        vErrors.push(err31);
                                                      }
                                                      errors++;
                                                    }
                                                    if ("generating_f16_gguf" !== data12) {
                                                      const err32 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/9/const", keyword: "const", params: { allowedValue: "generating_f16_gguf" }, message: "must be equal to constant" };
                                                      if (vErrors === null) {
                                                        vErrors = [err32];
                                                      } else {
                                                        vErrors.push(err32);
                                                      }
                                                      errors++;
                                                    }
                                                    var _valid1 = _errs60 === errors;
                                                    if (_valid1 && valid4) {
                                                      valid4 = false;
                                                      passing1 = [passing1, 9];
                                                    } else {
                                                      if (_valid1) {
                                                        valid4 = true;
                                                        passing1 = 9;
                                                      }
                                                      const _errs62 = errors;
                                                      if (typeof data12 !== "string") {
                                                        const err33 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/10/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                        if (vErrors === null) {
                                                          vErrors = [err33];
                                                        } else {
                                                          vErrors.push(err33);
                                                        }
                                                        errors++;
                                                      }
                                                      if ("computing_imatrix" !== data12) {
                                                        const err34 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/10/const", keyword: "const", params: { allowedValue: "computing_imatrix" }, message: "must be equal to constant" };
                                                        if (vErrors === null) {
                                                          vErrors = [err34];
                                                        } else {
                                                          vErrors.push(err34);
                                                        }
                                                        errors++;
                                                      }
                                                      var _valid1 = _errs62 === errors;
                                                      if (_valid1 && valid4) {
                                                        valid4 = false;
                                                        passing1 = [passing1, 10];
                                                      } else {
                                                        if (_valid1) {
                                                          valid4 = true;
                                                          passing1 = 10;
                                                        }
                                                        const _errs64 = errors;
                                                        if (typeof data12 !== "string") {
                                                          const err35 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/11/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                          if (vErrors === null) {
                                                            vErrors = [err35];
                                                          } else {
                                                            vErrors.push(err35);
                                                          }
                                                          errors++;
                                                        }
                                                        if ("quantizing" !== data12) {
                                                          const err36 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/11/const", keyword: "const", params: { allowedValue: "quantizing" }, message: "must be equal to constant" };
                                                          if (vErrors === null) {
                                                            vErrors = [err36];
                                                          } else {
                                                            vErrors.push(err36);
                                                          }
                                                          errors++;
                                                        }
                                                        var _valid1 = _errs64 === errors;
                                                        if (_valid1 && valid4) {
                                                          valid4 = false;
                                                          passing1 = [passing1, 11];
                                                        } else {
                                                          if (_valid1) {
                                                            valid4 = true;
                                                            passing1 = 11;
                                                          }
                                                          const _errs66 = errors;
                                                          if (typeof data12 !== "string") {
                                                            const err37 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/12/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                            if (vErrors === null) {
                                                              vErrors = [err37];
                                                            } else {
                                                              vErrors.push(err37);
                                                            }
                                                            errors++;
                                                          }
                                                          if ("calibrating" !== data12) {
                                                            const err38 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/12/const", keyword: "const", params: { allowedValue: "calibrating" }, message: "must be equal to constant" };
                                                            if (vErrors === null) {
                                                              vErrors = [err38];
                                                            } else {
                                                              vErrors.push(err38);
                                                            }
                                                            errors++;
                                                          }
                                                          var _valid1 = _errs66 === errors;
                                                          if (_valid1 && valid4) {
                                                            valid4 = false;
                                                            passing1 = [passing1, 12];
                                                          } else {
                                                            if (_valid1) {
                                                              valid4 = true;
                                                              passing1 = 12;
                                                            }
                                                            const _errs68 = errors;
                                                            if (typeof data12 !== "string") {
                                                              const err39 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/13/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                                              if (vErrors === null) {
                                                                vErrors = [err39];
                                                              } else {
                                                                vErrors.push(err39);
                                                              }
                                                              errors++;
                                                            }
                                                            if ("training" !== data12) {
                                                              const err40 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf/13/const", keyword: "const", params: { allowedValue: "training" }, message: "must be equal to constant" };
                                                              if (vErrors === null) {
                                                                vErrors = [err40];
                                                              } else {
                                                                vErrors.push(err40);
                                                              }
                                                              errors++;
                                                            }
                                                            var _valid1 = _errs68 === errors;
                                                            if (_valid1 && valid4) {
                                                              valid4 = false;
                                                              passing1 = [passing1, 13];
                                                            } else {
                                                              if (_valid1) {
                                                                valid4 = true;
                                                                passing1 = 13;
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
                                    if (!valid4) {
                                      const err41 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionStatus/oneOf", keyword: "oneOf", params: { passingSchemas: passing1 }, message: "must match exactly one schema in oneOf" };
                                      if (vErrors === null) {
                                        vErrors = [err41];
                                      } else {
                                        vErrors.push(err41);
                                      }
                                      errors++;
                                      validate24.errors = vErrors;
                                      return false;
                                    } else {
                                      errors = _errs41;
                                      if (vErrors !== null) {
                                        if (_errs41) {
                                          vErrors.length = _errs41;
                                        } else {
                                          vErrors = null;
                                        }
                                      }
                                    }
                                    var valid0 = _errs39 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.targetQuant !== void 0) {
                                      let data13 = data.targetQuant;
                                      const _errs70 = errors;
                                      if (typeof data13 !== "string" && data13 !== null) {
                                        validate24.errors = [{ instancePath: instancePath + "/targetQuant", schemaPath: "#/properties/targetQuant/type", keyword: "type", params: { type: schema27.properties.targetQuant.type }, message: "must be string,null" }];
                                        return false;
                                      }
                                      var valid0 = _errs70 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                    if (valid0) {
                                      if (data.tensorsCompleted !== void 0) {
                                        let data14 = data.tensorsCompleted;
                                        const _errs72 = errors;
                                        if (!(typeof data14 == "number" && (!(data14 % 1) && !isNaN(data14)) && isFinite(data14)) && data14 !== null) {
                                          validate24.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/type", keyword: "type", params: { type: schema27.properties.tensorsCompleted.type }, message: "must be integer,null" }];
                                          return false;
                                        }
                                        if (errors === _errs72) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 4294967295 || isNaN(data14)) {
                                              validate24.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate24.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                return false;
                                              }
                                            }
                                          }
                                        }
                                        var valid0 = _errs72 === errors;
                                      } else {
                                        var valid0 = true;
                                      }
                                      if (valid0) {
                                        if (data.tensorsTotal !== void 0) {
                                          let data15 = data.tensorsTotal;
                                          const _errs74 = errors;
                                          if (!(typeof data15 == "number" && (!(data15 % 1) && !isNaN(data15)) && isFinite(data15)) && data15 !== null) {
                                            validate24.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/type", keyword: "type", params: { type: schema27.properties.tensorsTotal.type }, message: "must be integer,null" }];
                                            return false;
                                          }
                                          if (errors === _errs74) {
                                            if (typeof data15 == "number" && isFinite(data15)) {
                                              if (data15 > 4294967295 || isNaN(data15)) {
                                                validate24.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                                return false;
                                              } else {
                                                if (data15 < 0 || isNaN(data15)) {
                                                  validate24.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                  return false;
                                                }
                                              }
                                            }
                                          }
                                          var valid0 = _errs74 === errors;
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
    } else {
      validate24.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate24.errors = vErrors;
  return errors === 0;
}
function validate23(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.progress === void 0 && (missing0 = "progress")) {
        validate23.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "progress" || key0 === "success")) {
            validate23.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.progress !== void 0) {
            let data0 = data.progress;
            const _errs2 = errors;
            const _errs3 = errors;
            let valid1 = false;
            const _errs4 = errors;
            if (!validate24(data0, { instancePath: instancePath + "/progress", parentData: data, parentDataProperty: "progress", rootData })) {
              vErrors = vErrors === null ? validate24.errors : vErrors.concat(validate24.errors);
              errors = vErrors.length;
            }
            var _valid0 = _errs4 === errors;
            valid1 = valid1 || _valid0;
            if (!valid1) {
              const _errs5 = errors;
              if (data0 !== null) {
                const err0 = { instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
                if (vErrors === null) {
                  vErrors = [err0];
                } else {
                  vErrors.push(err0);
                }
                errors++;
              }
              var _valid0 = _errs5 === errors;
              valid1 = valid1 || _valid0;
            }
            if (!valid1) {
              const err1 = { instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
              if (vErrors === null) {
                vErrors = [err1];
              } else {
                vErrors.push(err1);
              }
              errors++;
              validate23.errors = vErrors;
              return false;
            } else {
              errors = _errs3;
              if (vErrors !== null) {
                if (_errs3) {
                  vErrors.length = _errs3;
                } else {
                  vErrors = null;
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
              const _errs7 = errors;
              if (typeof data1 !== "boolean") {
                validate23.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate23.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                return false;
              }
              var valid0 = _errs7 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate23.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate23.errors = vErrors;
  return errors === 0;
}
var validateConversionSetupStartedOutcome = validate26;
var schema31 = { "additionalProperties": false, "properties": { "error": { "enum": [null, "Conversion environment setup did not complete successfully."], "type": ["string", "null"] }, "operationId": { "maxLength": 36, "minLength": 36, "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "type": "string" }, "status": { "$ref": "#/definitions/ConversionSetupStatus" } }, "pumasConversionSetup": true, "required": ["operationId", "status", "error"], "type": "object" };
var pattern12 = new RegExp("^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "u");
function validate27(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.operationId === void 0 && (missing0 = "operationId") || data.status === void 0 && (missing0 = "status") || data.error === void 0 && (missing0 = "error")) {
        validate27.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "error" || key0 === "operationId" || key0 === "status")) {
            validate27.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.error !== void 0) {
            let data0 = data.error;
            const _errs2 = errors;
            if (typeof data0 !== "string" && data0 !== null) {
              validate27.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema31.properties.error.type }, message: "must be string,null" }];
              return false;
            }
            if (!(data0 === null || data0 === "Conversion environment setup did not complete successfully.")) {
              validate27.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/enum", keyword: "enum", params: { allowedValues: schema31.properties.error.enum }, message: "must be equal to one of the allowed values" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.operationId !== void 0) {
              let data1 = data.operationId;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (typeof data1 === "string") {
                  if (func4(data1) > 36) {
                    validate27.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/maxLength", keyword: "maxLength", params: { limit: 36 }, message: "must NOT have more than 36 characters" }];
                    return false;
                  } else {
                    if (func4(data1) < 36) {
                      validate27.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/minLength", keyword: "minLength", params: { limit: 36 }, message: "must NOT have fewer than 36 characters" }];
                      return false;
                    } else {
                      if (!pattern12.test(data1)) {
                        validate27.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/pattern", keyword: "pattern", params: { pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$" }, message: 'must match pattern "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"' }];
                        return false;
                      }
                    }
                  }
                } else {
                  validate27.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                  return false;
                }
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.status !== void 0) {
                let data2 = data.status;
                const _errs6 = errors;
                const _errs8 = errors;
                let valid2 = false;
                let passing0 = null;
                const _errs9 = errors;
                if (typeof data2 !== "string") {
                  const err0 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err0];
                  } else {
                    vErrors.push(err0);
                  }
                  errors++;
                }
                if ("in_progress" !== data2) {
                  const err1 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/0/const", keyword: "const", params: { allowedValue: "in_progress" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err1];
                  } else {
                    vErrors.push(err1);
                  }
                  errors++;
                }
                var _valid0 = _errs9 === errors;
                if (_valid0) {
                  valid2 = true;
                  passing0 = 0;
                }
                const _errs11 = errors;
                if (typeof data2 !== "string") {
                  const err2 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err2];
                  } else {
                    vErrors.push(err2);
                  }
                  errors++;
                }
                if ("completed" !== data2) {
                  const err3 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/1/const", keyword: "const", params: { allowedValue: "completed" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err3];
                  } else {
                    vErrors.push(err3);
                  }
                  errors++;
                }
                var _valid0 = _errs11 === errors;
                if (_valid0 && valid2) {
                  valid2 = false;
                  passing0 = [passing0, 1];
                } else {
                  if (_valid0) {
                    valid2 = true;
                    passing0 = 1;
                  }
                  const _errs13 = errors;
                  if (typeof data2 !== "string") {
                    const err4 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err4];
                    } else {
                      vErrors.push(err4);
                    }
                    errors++;
                  }
                  if ("failed" !== data2) {
                    const err5 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/2/const", keyword: "const", params: { allowedValue: "failed" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err5];
                    } else {
                      vErrors.push(err5);
                    }
                    errors++;
                  }
                  var _valid0 = _errs13 === errors;
                  if (_valid0 && valid2) {
                    valid2 = false;
                    passing0 = [passing0, 2];
                  } else {
                    if (_valid0) {
                      valid2 = true;
                      passing0 = 2;
                    }
                    const _errs15 = errors;
                    if (typeof data2 !== "string") {
                      const err6 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err6];
                      } else {
                        vErrors.push(err6);
                      }
                      errors++;
                    }
                    if ("cancelled" !== data2) {
                      const err7 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/3/const", keyword: "const", params: { allowedValue: "cancelled" }, message: "must be equal to constant" };
                      if (vErrors === null) {
                        vErrors = [err7];
                      } else {
                        vErrors.push(err7);
                      }
                      errors++;
                    }
                    var _valid0 = _errs15 === errors;
                    if (_valid0 && valid2) {
                      valid2 = false;
                      passing0 = [passing0, 3];
                    } else {
                      if (_valid0) {
                        valid2 = true;
                        passing0 = 3;
                      }
                    }
                  }
                }
                if (!valid2) {
                  const err8 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
                  if (vErrors === null) {
                    vErrors = [err8];
                  } else {
                    vErrors.push(err8);
                  }
                  errors++;
                  validate27.errors = vErrors;
                  return false;
                } else {
                  errors = _errs8;
                  if (vErrors !== null) {
                    if (_errs8) {
                      vErrors.length = _errs8;
                    } else {
                      vErrors = null;
                    }
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.status === "failed" !== (data.error !== null)) {
                  validate27.errors = [{ instancePath, schemaPath: "#/pumasConversionSetup", keyword: "pumasConversionSetup", params: {}, message: 'must pass "pumasConversionSetup" keyword validation' }];
                  return false;
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
      if (data.success === void 0 && (missing0 = "success") || data.setup === void 0 && (missing0 = "setup")) {
        validate26.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "setup" || key0 === "success")) {
            validate26.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.setup !== void 0) {
            const _errs2 = errors;
            if (!validate27(data.setup, { instancePath: instancePath + "/setup", parentData: data, parentDataProperty: "setup", rootData })) {
              vErrors = vErrors === null ? validate27.errors : vErrors.concat(validate27.errors);
              errors = vErrors.length;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.success !== void 0) {
              let data1 = data.success;
              const _errs3 = errors;
              if (typeof data1 !== "boolean") {
                validate26.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate26.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                return false;
              }
              var valid0 = _errs3 === errors;
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
var validateConversionSetupStatusOutcome = validate29;
var schema34 = { "additionalProperties": false, "properties": { "error": { "enum": [null, "Conversion environment setup did not complete successfully."], "type": ["string", "null"] }, "operationId": { "maxLength": 36, "minLength": 36, "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "type": "string" }, "status": { "$ref": "#/definitions/ConversionSetupStatus" } }, "pumasConversionSetup": true, "required": ["operationId", "status", "error"], "type": "object" };
function validate30(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.operationId === void 0 && (missing0 = "operationId") || data.status === void 0 && (missing0 = "status") || data.error === void 0 && (missing0 = "error")) {
        validate30.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "error" || key0 === "operationId" || key0 === "status")) {
            validate30.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.error !== void 0) {
            let data0 = data.error;
            const _errs2 = errors;
            if (typeof data0 !== "string" && data0 !== null) {
              validate30.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema34.properties.error.type }, message: "must be string,null" }];
              return false;
            }
            if (!(data0 === null || data0 === "Conversion environment setup did not complete successfully.")) {
              validate30.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/enum", keyword: "enum", params: { allowedValues: schema34.properties.error.enum }, message: "must be equal to one of the allowed values" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.operationId !== void 0) {
              let data1 = data.operationId;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (typeof data1 === "string") {
                  if (func4(data1) > 36) {
                    validate30.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/maxLength", keyword: "maxLength", params: { limit: 36 }, message: "must NOT have more than 36 characters" }];
                    return false;
                  } else {
                    if (func4(data1) < 36) {
                      validate30.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/minLength", keyword: "minLength", params: { limit: 36 }, message: "must NOT have fewer than 36 characters" }];
                      return false;
                    } else {
                      if (!pattern12.test(data1)) {
                        validate30.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/pattern", keyword: "pattern", params: { pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$" }, message: 'must match pattern "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"' }];
                        return false;
                      }
                    }
                  }
                } else {
                  validate30.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                  return false;
                }
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.status !== void 0) {
                let data2 = data.status;
                const _errs6 = errors;
                const _errs8 = errors;
                let valid2 = false;
                let passing0 = null;
                const _errs9 = errors;
                if (typeof data2 !== "string") {
                  const err0 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err0];
                  } else {
                    vErrors.push(err0);
                  }
                  errors++;
                }
                if ("in_progress" !== data2) {
                  const err1 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/0/const", keyword: "const", params: { allowedValue: "in_progress" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err1];
                  } else {
                    vErrors.push(err1);
                  }
                  errors++;
                }
                var _valid0 = _errs9 === errors;
                if (_valid0) {
                  valid2 = true;
                  passing0 = 0;
                }
                const _errs11 = errors;
                if (typeof data2 !== "string") {
                  const err2 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err2];
                  } else {
                    vErrors.push(err2);
                  }
                  errors++;
                }
                if ("completed" !== data2) {
                  const err3 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/1/const", keyword: "const", params: { allowedValue: "completed" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err3];
                  } else {
                    vErrors.push(err3);
                  }
                  errors++;
                }
                var _valid0 = _errs11 === errors;
                if (_valid0 && valid2) {
                  valid2 = false;
                  passing0 = [passing0, 1];
                } else {
                  if (_valid0) {
                    valid2 = true;
                    passing0 = 1;
                  }
                  const _errs13 = errors;
                  if (typeof data2 !== "string") {
                    const err4 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err4];
                    } else {
                      vErrors.push(err4);
                    }
                    errors++;
                  }
                  if ("failed" !== data2) {
                    const err5 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/2/const", keyword: "const", params: { allowedValue: "failed" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err5];
                    } else {
                      vErrors.push(err5);
                    }
                    errors++;
                  }
                  var _valid0 = _errs13 === errors;
                  if (_valid0 && valid2) {
                    valid2 = false;
                    passing0 = [passing0, 2];
                  } else {
                    if (_valid0) {
                      valid2 = true;
                      passing0 = 2;
                    }
                    const _errs15 = errors;
                    if (typeof data2 !== "string") {
                      const err6 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err6];
                      } else {
                        vErrors.push(err6);
                      }
                      errors++;
                    }
                    if ("cancelled" !== data2) {
                      const err7 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf/3/const", keyword: "const", params: { allowedValue: "cancelled" }, message: "must be equal to constant" };
                      if (vErrors === null) {
                        vErrors = [err7];
                      } else {
                        vErrors.push(err7);
                      }
                      errors++;
                    }
                    var _valid0 = _errs15 === errors;
                    if (_valid0 && valid2) {
                      valid2 = false;
                      passing0 = [passing0, 3];
                    } else {
                      if (_valid0) {
                        valid2 = true;
                        passing0 = 3;
                      }
                    }
                  }
                }
                if (!valid2) {
                  const err8 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/ConversionSetupStatus/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
                  if (vErrors === null) {
                    vErrors = [err8];
                  } else {
                    vErrors.push(err8);
                  }
                  errors++;
                  validate30.errors = vErrors;
                  return false;
                } else {
                  errors = _errs8;
                  if (vErrors !== null) {
                    if (_errs8) {
                      vErrors.length = _errs8;
                    } else {
                      vErrors = null;
                    }
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.status === "failed" !== (data.error !== null)) {
                  validate30.errors = [{ instancePath, schemaPath: "#/pumasConversionSetup", keyword: "pumasConversionSetup", params: {}, message: 'must pass "pumasConversionSetup" keyword validation' }];
                  return false;
                }
              }
            }
          }
        }
      }
    } else {
      validate30.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate30.errors = vErrors;
  return errors === 0;
}
function validate29(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.setup === void 0 && (missing0 = "setup")) {
        validate29.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "setup" || key0 === "success")) {
            validate29.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.setup !== void 0) {
            let data0 = data.setup;
            const _errs2 = errors;
            const _errs3 = errors;
            let valid1 = false;
            const _errs4 = errors;
            if (!validate30(data0, { instancePath: instancePath + "/setup", parentData: data, parentDataProperty: "setup", rootData })) {
              vErrors = vErrors === null ? validate30.errors : vErrors.concat(validate30.errors);
              errors = vErrors.length;
            }
            var _valid0 = _errs4 === errors;
            valid1 = valid1 || _valid0;
            if (!valid1) {
              const _errs5 = errors;
              if (data0 !== null) {
                const err0 = { instancePath: instancePath + "/setup", schemaPath: "#/properties/setup/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
                if (vErrors === null) {
                  vErrors = [err0];
                } else {
                  vErrors.push(err0);
                }
                errors++;
              }
              var _valid0 = _errs5 === errors;
              valid1 = valid1 || _valid0;
            }
            if (!valid1) {
              const err1 = { instancePath: instancePath + "/setup", schemaPath: "#/properties/setup/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
              if (vErrors === null) {
                vErrors = [err1];
              } else {
                vErrors.push(err1);
              }
              errors++;
              validate29.errors = vErrors;
              return false;
            } else {
              errors = _errs3;
              if (vErrors !== null) {
                if (_errs3) {
                  vErrors.length = _errs3;
                } else {
                  vErrors = null;
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
              const _errs7 = errors;
              if (typeof data1 !== "boolean") {
                validate29.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate29.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                return false;
              }
              var valid0 = _errs7 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate29.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate29.errors = vErrors;
  return errors === 0;
}
var validateConversionStartedOutcome = validate32;
var pattern14 = new RegExp("[^\\t\\n\\v\\f\\r \\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000]", "u");
function validate32(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.conversion_id === void 0 && (missing0 = "conversion_id")) {
        validate32.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "conversion_id" || key0 === "success")) {
            validate32.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.conversion_id !== void 0) {
            let data0 = data.conversion_id;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (typeof data0 === "string") {
                if (!pattern14.test(data0)) {
                  validate32.errors = [{ instancePath: instancePath + "/conversion_id", schemaPath: "#/properties/conversion_id/pattern", keyword: "pattern", params: { pattern: "[^\\t\\n\\v\\f\\r \\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000]" }, message: 'must match pattern "[^\\t\\n\\v\\f\\r \\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000]"' }];
                  return false;
                } else {
                  if (encodeURIComponent(data0).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                    validate32.errors = [{ instancePath: instancePath + "/conversion_id", schemaPath: "#/properties/conversion_id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate32.errors = [{ instancePath: instancePath + "/conversion_id", schemaPath: "#/properties/conversion_id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
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
                validate32.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate32.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate32.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate32.errors = vErrors;
  return errors === 0;
}
var validateDownloadIdParams = validate33;
function validate33(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.download_id === void 0 && (missing0 = "download_id")) {
        validate33.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "download_id")) {
            validate33.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  validate33.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                  return false;
                } else {
                  if (encodeURIComponent(data0).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                    validate33.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate33.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
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
var validateDownloadListOutcome = validate34;
var schema39 = { "additionalProperties": false, "properties": { "downloadId": { "type": "string" }, "downloadedBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "error": { "type": ["string", "null"] }, "etaSeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "libraryModelId": { "pumasPortablePath": true, "pumasUtf8Max": 4096, "type": ["string", "null"] }, "modelName": { "type": ["string", "null"] }, "modelType": { "type": ["string", "null"] }, "nextRetryDelaySeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "repoId": { "type": ["string", "null"] }, "retryAttempt": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retryLimit": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retrying": { "type": ["boolean", "null"] }, "selectedArtifactId": { "type": ["string", "null"] }, "speed": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "status": { "$ref": "#/definitions/DownloadStatus" }, "totalBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["downloadId", "libraryModelId", "repoId", "selectedArtifactId", "modelName", "modelType", "status", "progress", "downloadedBytes", "totalBytes", "speed", "etaSeconds", "retryAttempt", "retryLimit", "retrying", "nextRetryDelaySeconds", "error"], "type": "object" };
var schema40 = { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" };
function validate35(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.downloadId === void 0 && (missing0 = "downloadId") || data.libraryModelId === void 0 && (missing0 = "libraryModelId") || data.repoId === void 0 && (missing0 = "repoId") || data.selectedArtifactId === void 0 && (missing0 = "selectedArtifactId") || data.modelName === void 0 && (missing0 = "modelName") || data.modelType === void 0 && (missing0 = "modelType") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.downloadedBytes === void 0 && (missing0 = "downloadedBytes") || data.totalBytes === void 0 && (missing0 = "totalBytes") || data.speed === void 0 && (missing0 = "speed") || data.etaSeconds === void 0 && (missing0 = "etaSeconds") || data.retryAttempt === void 0 && (missing0 = "retryAttempt") || data.retryLimit === void 0 && (missing0 = "retryLimit") || data.retrying === void 0 && (missing0 = "retrying") || data.nextRetryDelaySeconds === void 0 && (missing0 = "nextRetryDelaySeconds") || data.error === void 0 && (missing0 = "error")) {
        validate35.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema39.properties, key0)) {
            validate35.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.downloadId !== void 0) {
            const _errs2 = errors;
            if (typeof data.downloadId !== "string") {
              validate35.errors = [{ instancePath: instancePath + "/downloadId", schemaPath: "#/properties/downloadId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                validate35.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/type", keyword: "type", params: { type: schema39.properties.downloadedBytes.type }, message: "must be integer,null" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 9007199254740991 || isNaN(data1)) {
                    validate35.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate35.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                  validate35.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema39.properties.error.type }, message: "must be string,null" }];
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
                    validate35.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/type", keyword: "type", params: { type: schema39.properties.etaSeconds.type }, message: "must be number,null" }];
                    return false;
                  }
                  if (errors === _errs8) {
                    if (typeof data3 == "number" && isFinite(data3)) {
                      if (data3 > 17976931348623157e292 || isNaN(data3)) {
                        validate35.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                        return false;
                      } else {
                        if (data3 < 0 || isNaN(data3)) {
                          validate35.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate35.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/type", keyword: "type", params: { type: schema39.properties.libraryModelId.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (errors === _errs10) {
                      if (typeof data4 === "string") {
                        if (data4.length === 0 || data4.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(data4) || Array.from(data4).some((letter) => letter.codePointAt(0) < 32 || letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159) || data4.split("/").some((component) => {
                          const stem = component.split(".")[0].replace(/[a-z]/g, (letter) => letter.toUpperCase());
                          return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g, "x").length > 255 || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
                        })) {
                          validate35.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate35.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
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
                        validate35.errors = [{ instancePath: instancePath + "/modelName", schemaPath: "#/properties/modelName/type", keyword: "type", params: { type: schema39.properties.modelName.type }, message: "must be string,null" }];
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
                          validate35.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: schema39.properties.modelType.type }, message: "must be string,null" }];
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
                            validate35.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/type", keyword: "type", params: { type: schema39.properties.nextRetryDelaySeconds.type }, message: "must be number,null" }];
                            return false;
                          }
                          if (errors === _errs16) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 17976931348623157e292 || isNaN(data7)) {
                                validate35.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate35.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                              validate35.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema39.properties.progress.type }, message: "must be number,null" }];
                              return false;
                            }
                            if (errors === _errs18) {
                              if (typeof data8 == "number" && isFinite(data8)) {
                                if (data8 > 1 || isNaN(data8)) {
                                  validate35.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                  return false;
                                } else {
                                  if (data8 < 0 || isNaN(data8)) {
                                    validate35.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                validate35.errors = [{ instancePath: instancePath + "/repoId", schemaPath: "#/properties/repoId/type", keyword: "type", params: { type: schema39.properties.repoId.type }, message: "must be string,null" }];
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
                                  validate35.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/type", keyword: "type", params: { type: schema39.properties.retryAttempt.type }, message: "must be integer,null" }];
                                  return false;
                                }
                                if (errors === _errs22) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 4294967295 || isNaN(data10)) {
                                      validate35.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate35.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate35.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/type", keyword: "type", params: { type: schema39.properties.retryLimit.type }, message: "must be integer,null" }];
                                    return false;
                                  }
                                  if (errors === _errs24) {
                                    if (typeof data11 == "number" && isFinite(data11)) {
                                      if (data11 > 4294967295 || isNaN(data11)) {
                                        validate35.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                        return false;
                                      } else {
                                        if (data11 < 0 || isNaN(data11)) {
                                          validate35.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                      validate35.errors = [{ instancePath: instancePath + "/retrying", schemaPath: "#/properties/retrying/type", keyword: "type", params: { type: schema39.properties.retrying.type }, message: "must be boolean,null" }];
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
                                        validate35.errors = [{ instancePath: instancePath + "/selectedArtifactId", schemaPath: "#/properties/selectedArtifactId/type", keyword: "type", params: { type: schema39.properties.selectedArtifactId.type }, message: "must be string,null" }];
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
                                          validate35.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/type", keyword: "type", params: { type: schema39.properties.speed.type }, message: "must be number,null" }];
                                          return false;
                                        }
                                        if (errors === _errs30) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 17976931348623157e292 || isNaN(data14)) {
                                              validate35.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate35.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                            validate35.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                            return false;
                                          }
                                          if (!(data15 === "queued" || data15 === "downloading" || data15 === "pausing" || data15 === "paused" || data15 === "cancelling" || data15 === "completed" || data15 === "cancelled" || data15 === "error")) {
                                            validate35.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/enum", keyword: "enum", params: { allowedValues: schema40.enum }, message: "must be equal to one of the allowed values" }];
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
                                              validate35.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/type", keyword: "type", params: { type: schema39.properties.totalBytes.type }, message: "must be integer,null" }];
                                              return false;
                                            }
                                            if (errors === _errs35) {
                                              if (typeof data16 == "number" && isFinite(data16)) {
                                                if (data16 > 9007199254740991 || isNaN(data16)) {
                                                  validate35.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                                  return false;
                                                } else {
                                                  if (data16 < 0 || isNaN(data16)) {
                                                    validate35.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
      validate35.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate35.errors = vErrors;
  return errors === 0;
}
function validate34(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.downloads === void 0 && (missing0 = "downloads")) {
        validate34.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "downloads" || key0 === "success")) {
            validate34.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate35(data0[i0], { instancePath: instancePath + "/downloads/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate35.errors : vErrors.concat(validate35.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate34.errors = [{ instancePath: instancePath + "/downloads", schemaPath: "#/properties/downloads/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate34.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate34.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate34.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate34.errors = vErrors;
  return errors === 0;
}
var validateDownloadMutationOutcome = validate37;
function validate37(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success")) {
        validate37.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "error" || key0 === "success")) {
            validate37.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.error !== void 0) {
            const _errs2 = errors;
            if (typeof data.error !== "string") {
              validate37.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                validate37.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.success === true && data.error !== void 0 || data.success === false && typeof data.error !== "string") {
                validate37.errors = [{ instancePath, schemaPath: "#/pumasMutation", keyword: "pumasMutation", params: {}, message: 'must pass "pumasMutation" keyword validation' }];
                return false;
              }
            }
          }
        }
      }
    } else {
      validate37.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate37.errors = vErrors;
  return errors === 0;
}
var validateDownloadStartedOutcome = validate38;
var schema43 = { "additionalProperties": false, "properties": { "artifactId": { "type": ["string", "null"] }, "download_id": { "type": "string" }, "selectedArtifactId": { "type": ["string", "null"] }, "success": { "const": true, "type": "boolean" } }, "pumasStarted": true, "required": ["success", "download_id", "selectedArtifactId", "artifactId"], "type": "object" };
function validate38(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
              const err2 = { instancePath: instancePath + "/artifactId", schemaPath: "#/definitions/DownloadStartedSuccess/properties/artifactId/type", keyword: "type", params: { type: schema43.properties.artifactId.type }, message: "must be string,null" };
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
                  const err4 = { instancePath: instancePath + "/selectedArtifactId", schemaPath: "#/definitions/DownloadStartedSuccess/properties/selectedArtifactId/type", keyword: "type", params: { type: schema43.properties.selectedArtifactId.type }, message: "must be string,null" };
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
    validate38.errors = vErrors;
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
  validate38.errors = vErrors;
  return errors === 0;
}
var validateDownloadStatusOutcome = validate39;
var schema46 = { "additionalProperties": false, "properties": { "downloadId": { "type": "string" }, "downloadedBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "error": { "type": ["string", "null"] }, "etaSeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "libraryModelId": { "pumasPortablePath": true, "pumasUtf8Max": 4096, "type": ["string", "null"] }, "modelName": { "type": ["string", "null"] }, "modelType": { "type": ["string", "null"] }, "nextRetryDelaySeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "repoId": { "type": ["string", "null"] }, "retryAttempt": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retryLimit": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retrying": { "type": ["boolean", "null"] }, "selectedArtifactId": { "type": ["string", "null"] }, "speed": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "status": { "$ref": "#/definitions/DownloadStatus" }, "success": { "const": true, "type": "boolean" }, "totalBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["success", "downloadId", "libraryModelId", "repoId", "selectedArtifactId", "modelName", "modelType", "status", "progress", "downloadedBytes", "totalBytes", "speed", "etaSeconds", "retryAttempt", "retryLimit", "retrying", "nextRetryDelaySeconds", "error"], "type": "object" };
var schema47 = { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" };
function validate40(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.downloadId === void 0 && (missing0 = "downloadId") || data.libraryModelId === void 0 && (missing0 = "libraryModelId") || data.repoId === void 0 && (missing0 = "repoId") || data.selectedArtifactId === void 0 && (missing0 = "selectedArtifactId") || data.modelName === void 0 && (missing0 = "modelName") || data.modelType === void 0 && (missing0 = "modelType") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.downloadedBytes === void 0 && (missing0 = "downloadedBytes") || data.totalBytes === void 0 && (missing0 = "totalBytes") || data.speed === void 0 && (missing0 = "speed") || data.etaSeconds === void 0 && (missing0 = "etaSeconds") || data.retryAttempt === void 0 && (missing0 = "retryAttempt") || data.retryLimit === void 0 && (missing0 = "retryLimit") || data.retrying === void 0 && (missing0 = "retrying") || data.nextRetryDelaySeconds === void 0 && (missing0 = "nextRetryDelaySeconds") || data.error === void 0 && (missing0 = "error")) {
        validate40.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema46.properties, key0)) {
            validate40.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.downloadId !== void 0) {
            const _errs2 = errors;
            if (typeof data.downloadId !== "string") {
              validate40.errors = [{ instancePath: instancePath + "/downloadId", schemaPath: "#/properties/downloadId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                validate40.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/type", keyword: "type", params: { type: schema46.properties.downloadedBytes.type }, message: "must be integer,null" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 9007199254740991 || isNaN(data1)) {
                    validate40.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate40.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                  validate40.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema46.properties.error.type }, message: "must be string,null" }];
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
                    validate40.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/type", keyword: "type", params: { type: schema46.properties.etaSeconds.type }, message: "must be number,null" }];
                    return false;
                  }
                  if (errors === _errs8) {
                    if (typeof data3 == "number" && isFinite(data3)) {
                      if (data3 > 17976931348623157e292 || isNaN(data3)) {
                        validate40.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                        return false;
                      } else {
                        if (data3 < 0 || isNaN(data3)) {
                          validate40.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate40.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/type", keyword: "type", params: { type: schema46.properties.libraryModelId.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (errors === _errs10) {
                      if (typeof data4 === "string") {
                        if (data4.length === 0 || data4.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(data4) || Array.from(data4).some((letter) => letter.codePointAt(0) < 32 || letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159) || data4.split("/").some((component) => {
                          const stem = component.split(".")[0].replace(/[a-z]/g, (letter) => letter.toUpperCase());
                          return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g, "x").length > 255 || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
                        })) {
                          validate40.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate40.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
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
                        validate40.errors = [{ instancePath: instancePath + "/modelName", schemaPath: "#/properties/modelName/type", keyword: "type", params: { type: schema46.properties.modelName.type }, message: "must be string,null" }];
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
                          validate40.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: schema46.properties.modelType.type }, message: "must be string,null" }];
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
                            validate40.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/type", keyword: "type", params: { type: schema46.properties.nextRetryDelaySeconds.type }, message: "must be number,null" }];
                            return false;
                          }
                          if (errors === _errs16) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 17976931348623157e292 || isNaN(data7)) {
                                validate40.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate40.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                              validate40.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema46.properties.progress.type }, message: "must be number,null" }];
                              return false;
                            }
                            if (errors === _errs18) {
                              if (typeof data8 == "number" && isFinite(data8)) {
                                if (data8 > 1 || isNaN(data8)) {
                                  validate40.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                  return false;
                                } else {
                                  if (data8 < 0 || isNaN(data8)) {
                                    validate40.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                validate40.errors = [{ instancePath: instancePath + "/repoId", schemaPath: "#/properties/repoId/type", keyword: "type", params: { type: schema46.properties.repoId.type }, message: "must be string,null" }];
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
                                  validate40.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/type", keyword: "type", params: { type: schema46.properties.retryAttempt.type }, message: "must be integer,null" }];
                                  return false;
                                }
                                if (errors === _errs22) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 4294967295 || isNaN(data10)) {
                                      validate40.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate40.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate40.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/type", keyword: "type", params: { type: schema46.properties.retryLimit.type }, message: "must be integer,null" }];
                                    return false;
                                  }
                                  if (errors === _errs24) {
                                    if (typeof data11 == "number" && isFinite(data11)) {
                                      if (data11 > 4294967295 || isNaN(data11)) {
                                        validate40.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                        return false;
                                      } else {
                                        if (data11 < 0 || isNaN(data11)) {
                                          validate40.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                      validate40.errors = [{ instancePath: instancePath + "/retrying", schemaPath: "#/properties/retrying/type", keyword: "type", params: { type: schema46.properties.retrying.type }, message: "must be boolean,null" }];
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
                                        validate40.errors = [{ instancePath: instancePath + "/selectedArtifactId", schemaPath: "#/properties/selectedArtifactId/type", keyword: "type", params: { type: schema46.properties.selectedArtifactId.type }, message: "must be string,null" }];
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
                                          validate40.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/type", keyword: "type", params: { type: schema46.properties.speed.type }, message: "must be number,null" }];
                                          return false;
                                        }
                                        if (errors === _errs30) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 17976931348623157e292 || isNaN(data14)) {
                                              validate40.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate40.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                            validate40.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                            return false;
                                          }
                                          if (!(data15 === "queued" || data15 === "downloading" || data15 === "pausing" || data15 === "paused" || data15 === "cancelling" || data15 === "completed" || data15 === "cancelled" || data15 === "error")) {
                                            validate40.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/enum", keyword: "enum", params: { allowedValues: schema47.enum }, message: "must be equal to one of the allowed values" }];
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
                                              validate40.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                              return false;
                                            }
                                            if (true !== data16) {
                                              validate40.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
                                                validate40.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/type", keyword: "type", params: { type: schema46.properties.totalBytes.type }, message: "must be integer,null" }];
                                                return false;
                                              }
                                              if (errors === _errs37) {
                                                if (typeof data17 == "number" && isFinite(data17)) {
                                                  if (data17 > 9007199254740991 || isNaN(data17)) {
                                                    validate40.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                                    return false;
                                                  } else {
                                                    if (data17 < 0 || isNaN(data17)) {
                                                      validate40.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
      validate40.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate40.errors = vErrors;
  return errors === 0;
}
function validate39(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (!validate40(data, { instancePath, parentData, parentDataProperty, rootData })) {
    vErrors = vErrors === null ? validate40.errors : vErrors.concat(validate40.errors);
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
    validate39.errors = vErrors;
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
  validate39.errors = vErrors;
  return errors === 0;
}
var validateGetBackendSetupParams = validate42;
function validate42(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.backend === void 0 && (missing0 = "backend")) {
        validate42.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backend")) {
            validate42.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.backend !== void 0) {
            let data0 = data.backend;
            const _errs4 = errors;
            let valid2 = false;
            let passing0 = null;
            const _errs5 = errors;
            if (typeof data0 !== "string") {
              const err0 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err0];
              } else {
                vErrors.push(err0);
              }
              errors++;
            }
            if ("python_conversion" !== data0) {
              const err1 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/0/const", keyword: "const", params: { allowedValue: "python_conversion" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err1];
              } else {
                vErrors.push(err1);
              }
              errors++;
            }
            var _valid0 = _errs5 === errors;
            if (_valid0) {
              valid2 = true;
              passing0 = 0;
            }
            const _errs7 = errors;
            if (typeof data0 !== "string") {
              const err2 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("llama_cpp" !== data0) {
              const err3 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/1/const", keyword: "const", params: { allowedValue: "llama_cpp" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
            var _valid0 = _errs7 === errors;
            if (_valid0 && valid2) {
              valid2 = false;
              passing0 = [passing0, 1];
            } else {
              if (_valid0) {
                valid2 = true;
                passing0 = 1;
              }
              const _errs9 = errors;
              if (typeof data0 !== "string") {
                const err4 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err4];
                } else {
                  vErrors.push(err4);
                }
                errors++;
              }
              if ("nvfp4" !== data0) {
                const err5 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/2/const", keyword: "const", params: { allowedValue: "nvfp4" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err5];
                } else {
                  vErrors.push(err5);
                }
                errors++;
              }
              var _valid0 = _errs9 === errors;
              if (_valid0 && valid2) {
                valid2 = false;
                passing0 = [passing0, 2];
              } else {
                if (_valid0) {
                  valid2 = true;
                  passing0 = 2;
                }
                const _errs11 = errors;
                if (typeof data0 !== "string") {
                  const err6 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err6];
                  } else {
                    vErrors.push(err6);
                  }
                  errors++;
                }
                if ("sherry" !== data0) {
                  const err7 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/3/const", keyword: "const", params: { allowedValue: "sherry" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                }
                var _valid0 = _errs11 === errors;
                if (_valid0 && valid2) {
                  valid2 = false;
                  passing0 = [passing0, 3];
                } else {
                  if (_valid0) {
                    valid2 = true;
                    passing0 = 3;
                  }
                }
              }
            }
            if (!valid2) {
              const err8 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
              if (vErrors === null) {
                vErrors = [err8];
              } else {
                vErrors.push(err8);
              }
              errors++;
              validate42.errors = vErrors;
              return false;
            } else {
              errors = _errs4;
              if (vErrors !== null) {
                if (_errs4) {
                  vErrors.length = _errs4;
                } else {
                  vErrors = null;
                }
              }
            }
          }
        }
      }
    } else {
      validate42.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate42.errors = vErrors;
  return errors === 0;
}
var validateGetHfDownloadDetailsParams = validate43;
var schema51 = { "anyOf": [{ "additionalProperties": false, "properties": { "quants": { "default": null, "items": { "type": "string" }, "type": ["array", "null"] }, "repo_id": { "type": "string" } }, "required": ["repo_id"], "type": "object" }, { "additionalProperties": false, "properties": { "quants": { "default": null, "items": { "type": "string" }, "type": ["array", "null"] }, "repoId": { "type": "string" } }, "required": ["repoId"], "type": "object" }] };
function validate43(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.repo_id === void 0 && (missing0 = "repo_id")) {
        const err0 = { instancePath, schemaPath: "#/anyOf/0/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (!(key0 === "quants" || key0 === "repo_id")) {
            const err1 = { instancePath, schemaPath: "#/anyOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
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
          if (data.quants !== void 0) {
            let data0 = data.quants;
            const _errs4 = errors;
            if (!Array.isArray(data0) && data0 !== null) {
              const err2 = { instancePath: instancePath + "/quants", schemaPath: "#/anyOf/0/properties/quants/type", keyword: "type", params: { type: schema51.anyOf[0].properties.quants.type }, message: "must be array,null" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if (errors === _errs4) {
              if (Array.isArray(data0)) {
                var valid2 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs6 = errors;
                  if (typeof data0[i0] !== "string") {
                    const err3 = { instancePath: instancePath + "/quants/" + i0, schemaPath: "#/anyOf/0/properties/quants/items/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err3];
                    } else {
                      vErrors.push(err3);
                    }
                    errors++;
                  }
                  var valid2 = _errs6 === errors;
                  if (!valid2) {
                    break;
                  }
                }
              }
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.repo_id !== void 0) {
              const _errs8 = errors;
              if (typeof data.repo_id !== "string") {
                const err4 = { instancePath: instancePath + "/repo_id", schemaPath: "#/anyOf/0/properties/repo_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err4];
                } else {
                  vErrors.push(err4);
                }
                errors++;
              }
              var valid1 = _errs8 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      const err5 = { instancePath, schemaPath: "#/anyOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs10 = errors;
    if (errors === _errs10) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.repoId === void 0 && (missing1 = "repoId")) {
          const err6 = { instancePath, schemaPath: "#/anyOf/1/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err6];
          } else {
            vErrors.push(err6);
          }
          errors++;
        } else {
          const _errs12 = errors;
          for (const key1 in data) {
            if (!(key1 === "quants" || key1 === "repoId")) {
              const err7 = { instancePath, schemaPath: "#/anyOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err7];
              } else {
                vErrors.push(err7);
              }
              errors++;
              break;
            }
          }
          if (_errs12 === errors) {
            if (data.quants !== void 0) {
              let data3 = data.quants;
              const _errs13 = errors;
              if (!Array.isArray(data3) && data3 !== null) {
                const err8 = { instancePath: instancePath + "/quants", schemaPath: "#/anyOf/1/properties/quants/type", keyword: "type", params: { type: schema51.anyOf[1].properties.quants.type }, message: "must be array,null" };
                if (vErrors === null) {
                  vErrors = [err8];
                } else {
                  vErrors.push(err8);
                }
                errors++;
              }
              if (errors === _errs13) {
                if (Array.isArray(data3)) {
                  var valid4 = true;
                  const len1 = data3.length;
                  for (let i1 = 0; i1 < len1; i1++) {
                    const _errs15 = errors;
                    if (typeof data3[i1] !== "string") {
                      const err9 = { instancePath: instancePath + "/quants/" + i1, schemaPath: "#/anyOf/1/properties/quants/items/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err9];
                      } else {
                        vErrors.push(err9);
                      }
                      errors++;
                    }
                    var valid4 = _errs15 === errors;
                    if (!valid4) {
                      break;
                    }
                  }
                }
              }
              var valid3 = _errs13 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.repoId !== void 0) {
                const _errs17 = errors;
                if (typeof data.repoId !== "string") {
                  const err10 = { instancePath: instancePath + "/repoId", schemaPath: "#/anyOf/1/properties/repoId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err10];
                  } else {
                    vErrors.push(err10);
                  }
                  errors++;
                }
                var valid3 = _errs17 === errors;
              } else {
                var valid3 = true;
              }
            }
          }
        }
      } else {
        const err11 = { instancePath, schemaPath: "#/anyOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
    }
    var _valid0 = _errs10 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err12 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err12];
    } else {
      vErrors.push(err12);
    }
    errors++;
    validate43.errors = vErrors;
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
  validate43.errors = vErrors;
  return errors === 0;
}
var validateHfDownloadDetailsOutcome = validate44;
var schema54 = { "additionalProperties": false, "description": "Exact download details derived from a repository file tree.", "properties": { "downloadOptions": { "default": [], "items": { "$ref": "#/definitions/DownloadOption" }, "type": "array" }, "repoId": { "type": "string" }, "totalSizeBytes": { "default": null, "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["repoId", "downloadOptions", "totalSizeBytes"], "type": "object" };
var schema55 = { "additionalProperties": false, "description": "Download option for a quantization variant or file group.", "properties": { "fileGroup": { "$ref": "#/definitions/FileGroup" }, "quant": { "type": "string" }, "sizeBytes": { "default": null, "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["quant", "sizeBytes"], "type": "object" };
function validate47(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.quant === void 0 && (missing0 = "quant") || data.sizeBytes === void 0 && (missing0 = "sizeBytes")) {
        validate47.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "fileGroup" || key0 === "quant" || key0 === "sizeBytes")) {
            validate47.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.fileGroup !== void 0) {
            let data0 = data.fileGroup;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (data0 && typeof data0 == "object" && !Array.isArray(data0)) {
                let missing1;
                if (data0.filenames === void 0 && (missing1 = "filenames") || data0.shardCount === void 0 && (missing1 = "shardCount") || data0.label === void 0 && (missing1 = "label")) {
                  validate47.errors = [{ instancePath: instancePath + "/fileGroup", schemaPath: "#/definitions/FileGroup/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" }];
                  return false;
                } else {
                  const _errs5 = errors;
                  for (const key1 in data0) {
                    if (!(key1 === "filenames" || key1 === "label" || key1 === "shardCount")) {
                      validate47.errors = [{ instancePath: instancePath + "/fileGroup", schemaPath: "#/definitions/FileGroup/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" }];
                      return false;
                      break;
                    }
                  }
                  if (_errs5 === errors) {
                    if (data0.filenames !== void 0) {
                      let data1 = data0.filenames;
                      const _errs6 = errors;
                      if (errors === _errs6) {
                        if (Array.isArray(data1)) {
                          var valid3 = true;
                          const len0 = data1.length;
                          for (let i0 = 0; i0 < len0; i0++) {
                            const _errs8 = errors;
                            if (typeof data1[i0] !== "string") {
                              validate47.errors = [{ instancePath: instancePath + "/fileGroup/filenames/" + i0, schemaPath: "#/definitions/FileGroup/properties/filenames/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                              return false;
                            }
                            var valid3 = _errs8 === errors;
                            if (!valid3) {
                              break;
                            }
                          }
                        } else {
                          validate47.errors = [{ instancePath: instancePath + "/fileGroup/filenames", schemaPath: "#/definitions/FileGroup/properties/filenames/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                          return false;
                        }
                      }
                      var valid2 = _errs6 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data0.label !== void 0) {
                        const _errs10 = errors;
                        if (typeof data0.label !== "string") {
                          validate47.errors = [{ instancePath: instancePath + "/fileGroup/label", schemaPath: "#/definitions/FileGroup/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                          return false;
                        }
                        var valid2 = _errs10 === errors;
                      } else {
                        var valid2 = true;
                      }
                      if (valid2) {
                        if (data0.shardCount !== void 0) {
                          let data4 = data0.shardCount;
                          const _errs12 = errors;
                          if (!(typeof data4 == "number" && (!(data4 % 1) && !isNaN(data4)) && isFinite(data4))) {
                            validate47.errors = [{ instancePath: instancePath + "/fileGroup/shardCount", schemaPath: "#/definitions/FileGroup/properties/shardCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                            return false;
                          }
                          if (errors === _errs12) {
                            if (typeof data4 == "number" && isFinite(data4)) {
                              if (data4 > 4294967295 || isNaN(data4)) {
                                validate47.errors = [{ instancePath: instancePath + "/fileGroup/shardCount", schemaPath: "#/definitions/FileGroup/properties/shardCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                return false;
                              } else {
                                if (data4 < 0 || isNaN(data4)) {
                                  validate47.errors = [{ instancePath: instancePath + "/fileGroup/shardCount", schemaPath: "#/definitions/FileGroup/properties/shardCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                  return false;
                                }
                              }
                            }
                          }
                          var valid2 = _errs12 === errors;
                        } else {
                          var valid2 = true;
                        }
                      }
                    }
                  }
                }
              } else {
                validate47.errors = [{ instancePath: instancePath + "/fileGroup", schemaPath: "#/definitions/FileGroup/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.quant !== void 0) {
              const _errs14 = errors;
              if (typeof data.quant !== "string") {
                validate47.errors = [{ instancePath: instancePath + "/quant", schemaPath: "#/properties/quant/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
              var valid0 = _errs14 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.sizeBytes !== void 0) {
                let data6 = data.sizeBytes;
                const _errs16 = errors;
                if (!(typeof data6 == "number" && (!(data6 % 1) && !isNaN(data6)) && isFinite(data6)) && data6 !== null) {
                  validate47.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/type", keyword: "type", params: { type: schema55.properties.sizeBytes.type }, message: "must be integer,null" }];
                  return false;
                }
                if (errors === _errs16) {
                  if (typeof data6 == "number" && isFinite(data6)) {
                    if (data6 > 9007199254740991 || isNaN(data6)) {
                      validate47.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                      return false;
                    } else {
                      if (data6 < 0 || isNaN(data6)) {
                        validate47.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                        return false;
                      }
                    }
                  }
                }
                var valid0 = _errs16 === errors;
              } else {
                var valid0 = true;
              }
            }
          }
        }
      }
    } else {
      validate47.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate47.errors = vErrors;
  return errors === 0;
}
function validate46(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.repoId === void 0 && (missing0 = "repoId") || data.downloadOptions === void 0 && (missing0 = "downloadOptions") || data.totalSizeBytes === void 0 && (missing0 = "totalSizeBytes")) {
        validate46.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "downloadOptions" || key0 === "repoId" || key0 === "totalSizeBytes")) {
            validate46.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.downloadOptions !== void 0) {
            let data0 = data.downloadOptions;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (!validate47(data0[i0], { instancePath: instancePath + "/downloadOptions/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate47.errors : vErrors.concat(validate47.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate46.errors = [{ instancePath: instancePath + "/downloadOptions", schemaPath: "#/properties/downloadOptions/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.repoId !== void 0) {
              const _errs5 = errors;
              if (typeof data.repoId !== "string") {
                validate46.errors = [{ instancePath: instancePath + "/repoId", schemaPath: "#/properties/repoId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.totalSizeBytes !== void 0) {
                let data3 = data.totalSizeBytes;
                const _errs7 = errors;
                if (!(typeof data3 == "number" && (!(data3 % 1) && !isNaN(data3)) && isFinite(data3)) && data3 !== null) {
                  validate46.errors = [{ instancePath: instancePath + "/totalSizeBytes", schemaPath: "#/properties/totalSizeBytes/type", keyword: "type", params: { type: schema54.properties.totalSizeBytes.type }, message: "must be integer,null" }];
                  return false;
                }
                if (errors === _errs7) {
                  if (typeof data3 == "number" && isFinite(data3)) {
                    if (data3 > 9007199254740991 || isNaN(data3)) {
                      validate46.errors = [{ instancePath: instancePath + "/totalSizeBytes", schemaPath: "#/properties/totalSizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                      return false;
                    } else {
                      if (data3 < 0 || isNaN(data3)) {
                        validate46.errors = [{ instancePath: instancePath + "/totalSizeBytes", schemaPath: "#/properties/totalSizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                        return false;
                      }
                    }
                  }
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
      validate46.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate46.errors = vErrors;
  return errors === 0;
}
function validate45(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.details === void 0 && (missing0 = "details")) {
        validate45.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "details" || key0 === "success")) {
            validate45.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.details !== void 0) {
            const _errs2 = errors;
            if (!validate46(data.details, { instancePath: instancePath + "/details", parentData: data, parentDataProperty: "details", rootData })) {
              vErrors = vErrors === null ? validate46.errors : vErrors.concat(validate46.errors);
              errors = vErrors.length;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.success !== void 0) {
              let data1 = data.success;
              const _errs3 = errors;
              if (typeof data1 !== "boolean") {
                validate45.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate45.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                return false;
              }
              var valid0 = _errs3 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate45.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate45.errors = vErrors;
  return errors === 0;
}
function validate44(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (!validate45(data, { instancePath, parentData, parentDataProperty, rootData })) {
    vErrors = vErrors === null ? validate45.errors : vErrors.concat(validate45.errors);
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
          const err0 = { instancePath, schemaPath: "#/definitions/HfDownloadDetailsFailure/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
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
              const err1 = { instancePath, schemaPath: "#/definitions/HfDownloadDetailsFailure/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
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
                const err2 = { instancePath: instancePath + "/error", schemaPath: "#/definitions/HfDownloadDetailsFailure/properties/error/type", keyword: "type", params: { type: "string" }, message: "must be string" };
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
                  const err3 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/HfDownloadDetailsFailure/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                  if (vErrors === null) {
                    vErrors = [err3];
                  } else {
                    vErrors.push(err3);
                  }
                  errors++;
                }
                if (false !== data1) {
                  const err4 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/HfDownloadDetailsFailure/properties/success/const", keyword: "const", params: { allowedValue: false }, message: "must be equal to constant" };
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
        const err5 = { instancePath, schemaPath: "#/definitions/HfDownloadDetailsFailure/type", keyword: "type", params: { type: "object" }, message: "must be object" };
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
    validate44.errors = vErrors;
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
  validate44.errors = vErrors;
  return errors === 0;
}
var validateInferenceSettingsOutcome = validate51;
var schema59 = { "additionalProperties": false, "description": "Describes a single configurable inference parameter with its type,\ndefault value, and optional constraints.\n\nDownstream consumers (e.g. Pantograph node graph) use this schema\nto dynamically render UI controls for model-specific settings.", "properties": { "constraints": { "anyOf": [{ "$ref": "#/definitions/ParamConstraints" }, { "type": "null" }], "default": null, "description": "Optional numeric/enum constraints." }, "default": { "$ref": "#/definitions/InferenceSettingsJsonValue" }, "description": { "default": null, "description": "Optional description / tooltip.", "type": ["string", "null"] }, "key": { "description": 'Machine-readable key (e.g. "context_length", "denoising_steps").', "type": "string" }, "label": { "description": 'Human-readable label (e.g. "Context Length").', "type": "string" }, "param_type": { "allOf": [{ "$ref": "#/definitions/ParamType" }], "description": "Data type of this parameter." } }, "required": ["key", "label", "param_type", "default", "description", "constraints"], "type": "object" };
var schema62 = { "description": "Data type for an inference parameter.", "enum": ["Number", "Integer", "String", "Boolean"], "type": "string" };
var schema60 = { "additionalProperties": false, "description": "Constraints on an inference parameter value.", "properties": { "allowed_values": { "anyOf": [{ "type": "null" }, { "items": { "$ref": "#/definitions/InferenceSettingsJsonValue" }, "type": "array" }] }, "max": { "default": null, "maximum": 17976931348623157e292, "minimum": -17976931348623157e292, "type": ["number", "null"] }, "min": { "default": null, "maximum": 17976931348623157e292, "minimum": -17976931348623157e292, "type": ["number", "null"] } }, "required": ["min", "max", "allowed_values"], "type": "object" };
var wrapper0 = { validate: validate54 };
function validate54(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (data !== null) {
    const err0 = { instancePath, schemaPath: "#/anyOf/0/type", keyword: "type", params: { type: "null" }, message: "must be null" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs3 = errors;
    if (typeof data !== "boolean") {
      const err1 = { instancePath, schemaPath: "#/anyOf/1/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    var _valid0 = _errs3 === errors;
    valid0 = valid0 || _valid0;
    if (!valid0) {
      const _errs5 = errors;
      if (typeof data !== "string") {
        const err2 = { instancePath, schemaPath: "#/anyOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
      var _valid0 = _errs5 === errors;
      valid0 = valid0 || _valid0;
      if (!valid0) {
        const _errs7 = errors;
        if (errors === _errs7) {
          if (typeof data == "number" && isFinite(data)) {
            if (data > 9007199254740991 || isNaN(data)) {
              const err3 = { instancePath, schemaPath: "#/anyOf/3/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            } else {
              if (data < -9007199254740991 || isNaN(data)) {
                const err4 = { instancePath, schemaPath: "#/anyOf/3/minimum", keyword: "minimum", params: { comparison: ">=", limit: -9007199254740991 }, message: "must be >= -9007199254740991" };
                if (vErrors === null) {
                  vErrors = [err4];
                } else {
                  vErrors.push(err4);
                }
                errors++;
              }
            }
          } else {
            const err5 = { instancePath, schemaPath: "#/anyOf/3/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err5];
            } else {
              vErrors.push(err5);
            }
            errors++;
          }
        }
        var _valid0 = _errs7 === errors;
        valid0 = valid0 || _valid0;
        if (!valid0) {
          const _errs9 = errors;
          if (errors === _errs9) {
            if (Array.isArray(data)) {
              var valid1 = true;
              const len0 = data.length;
              for (let i0 = 0; i0 < len0; i0++) {
                const _errs11 = errors;
                if (!wrapper0.validate(data[i0], { instancePath: instancePath + "/" + i0, parentData: data, parentDataProperty: i0, rootData })) {
                  vErrors = vErrors === null ? wrapper0.validate.errors : vErrors.concat(wrapper0.validate.errors);
                  errors = vErrors.length;
                }
                var valid1 = _errs11 === errors;
                if (!valid1) {
                  break;
                }
              }
            } else {
              const err6 = { instancePath, schemaPath: "#/anyOf/4/type", keyword: "type", params: { type: "array" }, message: "must be array" };
              if (vErrors === null) {
                vErrors = [err6];
              } else {
                vErrors.push(err6);
              }
              errors++;
            }
          }
          var _valid0 = _errs9 === errors;
          valid0 = valid0 || _valid0;
          if (!valid0) {
            const _errs12 = errors;
            if (errors === _errs12) {
              if (data && typeof data == "object" && !Array.isArray(data)) {
                for (const key0 in data) {
                  const _errs15 = errors;
                  if (!wrapper0.validate(data[key0], { instancePath: instancePath + "/" + key0.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data, parentDataProperty: key0, rootData })) {
                    vErrors = vErrors === null ? wrapper0.validate.errors : vErrors.concat(wrapper0.validate.errors);
                    errors = vErrors.length;
                  }
                  var valid2 = _errs15 === errors;
                  if (!valid2) {
                    break;
                  }
                }
              } else {
                const err7 = { instancePath, schemaPath: "#/anyOf/5/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                if (vErrors === null) {
                  vErrors = [err7];
                } else {
                  vErrors.push(err7);
                }
                errors++;
              }
            }
            var _valid0 = _errs12 === errors;
            valid0 = valid0 || _valid0;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err8 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
    validate54.errors = vErrors;
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
  validate54.errors = vErrors;
  return errors === 0;
}
function validate53(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.min === void 0 && (missing0 = "min") || data.max === void 0 && (missing0 = "max") || data.allowed_values === void 0 && (missing0 = "allowed_values")) {
        validate53.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "allowed_values" || key0 === "max" || key0 === "min")) {
            validate53.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.allowed_values !== void 0) {
            let data0 = data.allowed_values;
            const _errs2 = errors;
            const _errs3 = errors;
            let valid1 = false;
            const _errs4 = errors;
            if (data0 !== null) {
              const err0 = { instancePath: instancePath + "/allowed_values", schemaPath: "#/properties/allowed_values/anyOf/0/type", keyword: "type", params: { type: "null" }, message: "must be null" };
              if (vErrors === null) {
                vErrors = [err0];
              } else {
                vErrors.push(err0);
              }
              errors++;
            }
            var _valid0 = _errs4 === errors;
            valid1 = valid1 || _valid0;
            if (!valid1) {
              const _errs6 = errors;
              if (errors === _errs6) {
                if (Array.isArray(data0)) {
                  var valid2 = true;
                  const len0 = data0.length;
                  for (let i0 = 0; i0 < len0; i0++) {
                    const _errs8 = errors;
                    if (!validate54(data0[i0], { instancePath: instancePath + "/allowed_values/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                      vErrors = vErrors === null ? validate54.errors : vErrors.concat(validate54.errors);
                      errors = vErrors.length;
                    }
                    var valid2 = _errs8 === errors;
                    if (!valid2) {
                      break;
                    }
                  }
                } else {
                  const err1 = { instancePath: instancePath + "/allowed_values", schemaPath: "#/properties/allowed_values/anyOf/1/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                  if (vErrors === null) {
                    vErrors = [err1];
                  } else {
                    vErrors.push(err1);
                  }
                  errors++;
                }
              }
              var _valid0 = _errs6 === errors;
              valid1 = valid1 || _valid0;
            }
            if (!valid1) {
              const err2 = { instancePath: instancePath + "/allowed_values", schemaPath: "#/properties/allowed_values/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
              validate53.errors = vErrors;
              return false;
            } else {
              errors = _errs3;
              if (vErrors !== null) {
                if (_errs3) {
                  vErrors.length = _errs3;
                } else {
                  vErrors = null;
                }
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.max !== void 0) {
              let data2 = data.max;
              const _errs9 = errors;
              if (!(typeof data2 == "number" && isFinite(data2)) && data2 !== null) {
                validate53.errors = [{ instancePath: instancePath + "/max", schemaPath: "#/properties/max/type", keyword: "type", params: { type: schema60.properties.max.type }, message: "must be number,null" }];
                return false;
              }
              if (errors === _errs9) {
                if (typeof data2 == "number" && isFinite(data2)) {
                  if (data2 > 17976931348623157e292 || isNaN(data2)) {
                    validate53.errors = [{ instancePath: instancePath + "/max", schemaPath: "#/properties/max/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                    return false;
                  } else {
                    if (data2 < -17976931348623157e292 || isNaN(data2)) {
                      validate53.errors = [{ instancePath: instancePath + "/max", schemaPath: "#/properties/max/minimum", keyword: "minimum", params: { comparison: ">=", limit: -17976931348623157e292 }, message: "must be >= -1.7976931348623157e+308" }];
                      return false;
                    }
                  }
                }
              }
              var valid0 = _errs9 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.min !== void 0) {
                let data3 = data.min;
                const _errs11 = errors;
                if (!(typeof data3 == "number" && isFinite(data3)) && data3 !== null) {
                  validate53.errors = [{ instancePath: instancePath + "/min", schemaPath: "#/properties/min/type", keyword: "type", params: { type: schema60.properties.min.type }, message: "must be number,null" }];
                  return false;
                }
                if (errors === _errs11) {
                  if (typeof data3 == "number" && isFinite(data3)) {
                    if (data3 > 17976931348623157e292 || isNaN(data3)) {
                      validate53.errors = [{ instancePath: instancePath + "/min", schemaPath: "#/properties/min/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                      return false;
                    } else {
                      if (data3 < -17976931348623157e292 || isNaN(data3)) {
                        validate53.errors = [{ instancePath: instancePath + "/min", schemaPath: "#/properties/min/minimum", keyword: "minimum", params: { comparison: ">=", limit: -17976931348623157e292 }, message: "must be >= -1.7976931348623157e+308" }];
                        return false;
                      }
                    }
                  }
                }
                var valid0 = _errs11 === errors;
              } else {
                var valid0 = true;
              }
            }
          }
        }
      }
    } else {
      validate53.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate53.errors = vErrors;
  return errors === 0;
}
function validate52(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.key === void 0 && (missing0 = "key") || data.label === void 0 && (missing0 = "label") || data.param_type === void 0 && (missing0 = "param_type") || data.default === void 0 && (missing0 = "default") || data.description === void 0 && (missing0 = "description") || data.constraints === void 0 && (missing0 = "constraints")) {
        validate52.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "constraints" || key0 === "default" || key0 === "description" || key0 === "key" || key0 === "label" || key0 === "param_type")) {
            validate52.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.constraints !== void 0) {
            let data0 = data.constraints;
            const _errs2 = errors;
            const _errs3 = errors;
            let valid1 = false;
            const _errs4 = errors;
            if (!validate53(data0, { instancePath: instancePath + "/constraints", parentData: data, parentDataProperty: "constraints", rootData })) {
              vErrors = vErrors === null ? validate53.errors : vErrors.concat(validate53.errors);
              errors = vErrors.length;
            }
            var _valid0 = _errs4 === errors;
            valid1 = valid1 || _valid0;
            if (!valid1) {
              const _errs5 = errors;
              if (data0 !== null) {
                const err0 = { instancePath: instancePath + "/constraints", schemaPath: "#/properties/constraints/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
                if (vErrors === null) {
                  vErrors = [err0];
                } else {
                  vErrors.push(err0);
                }
                errors++;
              }
              var _valid0 = _errs5 === errors;
              valid1 = valid1 || _valid0;
            }
            if (!valid1) {
              const err1 = { instancePath: instancePath + "/constraints", schemaPath: "#/properties/constraints/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
              if (vErrors === null) {
                vErrors = [err1];
              } else {
                vErrors.push(err1);
              }
              errors++;
              validate52.errors = vErrors;
              return false;
            } else {
              errors = _errs3;
              if (vErrors !== null) {
                if (_errs3) {
                  vErrors.length = _errs3;
                } else {
                  vErrors = null;
                }
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.default !== void 0) {
              const _errs7 = errors;
              if (!validate54(data.default, { instancePath: instancePath + "/default", parentData: data, parentDataProperty: "default", rootData })) {
                vErrors = vErrors === null ? validate54.errors : vErrors.concat(validate54.errors);
                errors = vErrors.length;
              }
              var valid0 = _errs7 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.description !== void 0) {
                let data2 = data.description;
                const _errs8 = errors;
                if (typeof data2 !== "string" && data2 !== null) {
                  validate52.errors = [{ instancePath: instancePath + "/description", schemaPath: "#/properties/description/type", keyword: "type", params: { type: schema59.properties.description.type }, message: "must be string,null" }];
                  return false;
                }
                var valid0 = _errs8 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.key !== void 0) {
                  const _errs10 = errors;
                  if (typeof data.key !== "string") {
                    validate52.errors = [{ instancePath: instancePath + "/key", schemaPath: "#/properties/key/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                  var valid0 = _errs10 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.label !== void 0) {
                    const _errs12 = errors;
                    if (typeof data.label !== "string") {
                      validate52.errors = [{ instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                      return false;
                    }
                    var valid0 = _errs12 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.param_type !== void 0) {
                      let data5 = data.param_type;
                      const _errs14 = errors;
                      if (typeof data5 !== "string") {
                        validate52.errors = [{ instancePath: instancePath + "/param_type", schemaPath: "#/definitions/ParamType/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                      if (!(data5 === "Number" || data5 === "Integer" || data5 === "String" || data5 === "Boolean")) {
                        validate52.errors = [{ instancePath: instancePath + "/param_type", schemaPath: "#/definitions/ParamType/enum", keyword: "enum", params: { allowedValues: schema62.enum }, message: "must be equal to one of the allowed values" }];
                        return false;
                      }
                      var valid0 = _errs14 === errors;
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
    } else {
      validate52.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate52.errors = vErrors;
  return errors === 0;
}
function validate51(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.model_id === void 0 && (missing0 = "model_id") || data.inference_settings === void 0 && (missing0 = "inference_settings")) {
        validate51.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "inference_settings" || key0 === "model_id" || key0 === "success")) {
            validate51.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.inference_settings !== void 0) {
            let data0 = data.inference_settings;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (!validate52(data0[i0], { instancePath: instancePath + "/inference_settings/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate52.errors : vErrors.concat(validate52.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate51.errors = [{ instancePath: instancePath + "/inference_settings", schemaPath: "#/properties/inference_settings/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.model_id !== void 0) {
              const _errs5 = errors;
              if (typeof data.model_id !== "string") {
                validate51.errors = [{ instancePath: instancePath + "/model_id", schemaPath: "#/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.success !== void 0) {
                let data3 = data.success;
                const _errs7 = errors;
                if (typeof data3 !== "boolean") {
                  validate51.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                  return false;
                }
                if (true !== data3) {
                  validate51.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate51.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate51.errors = vErrors;
  return errors === 0;
}
var validateLinkHealthOutcome = validate59;
var schema64 = { "additionalProperties": false, "description": "Link health response.\n\nNote: Not FFI-compatible due to `usize` fields. Use wrapper types in pumas-uniffi.", "properties": { "broken_links": { "items": { "type": "string" }, "type": "array" }, "error": { "type": "null" }, "errors": { "items": { "type": "string" }, "type": "array" }, "healthy_links": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" }, "orphaned_links": { "items": { "type": "string" }, "type": "array" }, "status": { "enum": ["healthy", "degraded"], "type": "string" }, "success": { "const": true, "type": "boolean" }, "total_links": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" }, "warnings": { "items": { "type": "string" }, "type": "array" } }, "pumasLinkHealth": true, "required": ["success", "status", "total_links", "healthy_links", "broken_links", "orphaned_links", "warnings", "errors"], "type": "object" };
function validate59(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.status === void 0 && (missing0 = "status") || data.total_links === void 0 && (missing0 = "total_links") || data.healthy_links === void 0 && (missing0 = "healthy_links") || data.broken_links === void 0 && (missing0 = "broken_links") || data.orphaned_links === void 0 && (missing0 = "orphaned_links") || data.warnings === void 0 && (missing0 = "warnings") || data.errors === void 0 && (missing0 = "errors")) {
        validate59.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (!func2.call(schema64.properties, key0)) {
            validate59.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                    validate59.errors = [{ instancePath: instancePath + "/broken_links/" + i0, schemaPath: "#/definitions/LinkHealthResponse/properties/broken_links/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                  var valid3 = _errs6 === errors;
                  if (!valid3) {
                    break;
                  }
                }
              } else {
                validate59.errors = [{ instancePath: instancePath + "/broken_links", schemaPath: "#/definitions/LinkHealthResponse/properties/broken_links/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate59.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/definitions/LinkHealthResponse/properties/error/type", keyword: "type", params: { type: "null" }, message: "must be null" }];
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
                        validate59.errors = [{ instancePath: instancePath + "/errors/" + i1, schemaPath: "#/definitions/LinkHealthResponse/properties/errors/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                      var valid4 = _errs12 === errors;
                      if (!valid4) {
                        break;
                      }
                    }
                  } else {
                    validate59.errors = [{ instancePath: instancePath + "/errors", schemaPath: "#/definitions/LinkHealthResponse/properties/errors/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                    validate59.errors = [{ instancePath: instancePath + "/healthy_links", schemaPath: "#/definitions/LinkHealthResponse/properties/healthy_links/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                    return false;
                  }
                  if (errors === _errs14) {
                    if (typeof data5 == "number" && isFinite(data5)) {
                      if (data5 > 9007199254740991 || isNaN(data5)) {
                        validate59.errors = [{ instancePath: instancePath + "/healthy_links", schemaPath: "#/definitions/LinkHealthResponse/properties/healthy_links/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                        return false;
                      } else {
                        if (data5 < 0 || isNaN(data5)) {
                          validate59.errors = [{ instancePath: instancePath + "/healthy_links", schemaPath: "#/definitions/LinkHealthResponse/properties/healthy_links/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                            validate59.errors = [{ instancePath: instancePath + "/orphaned_links/" + i2, schemaPath: "#/definitions/LinkHealthResponse/properties/orphaned_links/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                            return false;
                          }
                          var valid5 = _errs18 === errors;
                          if (!valid5) {
                            break;
                          }
                        }
                      } else {
                        validate59.errors = [{ instancePath: instancePath + "/orphaned_links", schemaPath: "#/definitions/LinkHealthResponse/properties/orphaned_links/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                        validate59.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/LinkHealthResponse/properties/status/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                      if (!(data8 === "healthy" || data8 === "degraded")) {
                        validate59.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/LinkHealthResponse/properties/status/enum", keyword: "enum", params: { allowedValues: schema64.properties.status.enum }, message: "must be equal to one of the allowed values" }];
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
                          validate59.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/definitions/LinkHealthResponse/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                          return false;
                        }
                        if (true !== data9) {
                          validate59.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/definitions/LinkHealthResponse/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
                            validate59.errors = [{ instancePath: instancePath + "/total_links", schemaPath: "#/definitions/LinkHealthResponse/properties/total_links/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                            return false;
                          }
                          if (errors === _errs24) {
                            if (typeof data10 == "number" && isFinite(data10)) {
                              if (data10 > 9007199254740991 || isNaN(data10)) {
                                validate59.errors = [{ instancePath: instancePath + "/total_links", schemaPath: "#/definitions/LinkHealthResponse/properties/total_links/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                return false;
                              } else {
                                if (data10 < 0 || isNaN(data10)) {
                                  validate59.errors = [{ instancePath: instancePath + "/total_links", schemaPath: "#/definitions/LinkHealthResponse/properties/total_links/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate59.errors = [{ instancePath: instancePath + "/warnings/" + i3, schemaPath: "#/definitions/LinkHealthResponse/properties/warnings/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                    return false;
                                  }
                                  var valid6 = _errs28 === errors;
                                  if (!valid6) {
                                    break;
                                  }
                                }
                              } else {
                                validate59.errors = [{ instancePath: instancePath + "/warnings", schemaPath: "#/definitions/LinkHealthResponse/properties/warnings/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                return false;
                              }
                            }
                            var valid2 = _errs26 === errors;
                          } else {
                            var valid2 = true;
                          }
                          if (valid2) {
                            if (!Array.isArray(data.broken_links) || data.healthy_links + data.broken_links.length !== data.total_links || data.status === "healthy" !== (data.broken_links.length === 0)) {
                              validate59.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/pumasLinkHealth", keyword: "pumasLinkHealth", params: {}, message: 'must pass "pumasLinkHealth" keyword validation' }];
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
      validate59.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (!Array.isArray(data.broken_links) || data.healthy_links + data.broken_links.length !== data.total_links || data.status === "healthy" !== (data.broken_links.length === 0)) {
        validate59.errors = [{ instancePath, schemaPath: "#/pumasLinkHealth", keyword: "pumasLinkHealth", params: {}, message: 'must pass "pumasLinkHealth" keyword validation' }];
        return false;
      }
    }
  }
  validate59.errors = vErrors;
  return errors === 0;
}
var validateModelIndexRefreshOutcome = validate60;
function validate60(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.indexed_count === void 0 && (missing0 = "indexed_count")) {
        validate60.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "indexed_count" || key0 === "success")) {
            validate60.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.indexed_count !== void 0) {
            let data0 = data.indexed_count;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0))) {
              validate60.errors = [{ instancePath: instancePath + "/indexed_count", schemaPath: "#/properties/indexed_count/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 4294967295 || isNaN(data0)) {
                  validate60.errors = [{ instancePath: instancePath + "/indexed_count", schemaPath: "#/properties/indexed_count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                  return false;
                } else {
                  if (data0 < 0 || isNaN(data0)) {
                    validate60.errors = [{ instancePath: instancePath + "/indexed_count", schemaPath: "#/properties/indexed_count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                validate60.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate60.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate60.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate60.errors = vErrors;
  return errors === 0;
}
var validateModelsOutcome = validate61;
var schema67 = { "additionalProperties": false, "properties": { "artifact": { "$ref": "#/definitions/CatalogArtifactState" }, "dependencyCount": { "maximum": 512, "minimum": 0, "type": "integer" }, "displayDate": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "displayName": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "format": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "id": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "integrity": { "$ref": "#/definitions/CatalogIntegrityState" }, "modelDir": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "modelType": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "quantization": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "relatedAvailable": { "type": "boolean" }, "sizeBytes": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" } }, "pumasCatalogRow": true, "required": ["id", "modelDir", "displayName", "modelType", "dependencyCount", "relatedAvailable", "artifact", "integrity"], "type": "object" };
var schema69 = { "enum": ["part_file_present", "expected_files_missing"], "type": "string" };
var pattern17 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern18 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern19 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate63(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
                          const err14 = { instancePath: instancePath + "/reasons/" + i0, schemaPath: "#/definitions/CatalogPartialReason/enum", keyword: "enum", params: { allowedValues: schema69.enum }, message: "must be equal to one of the allowed values" };
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
                                    if (data6.length === 0 || pattern17.test(data6)) {
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
                                    if (data9.length === 0 || pattern18.test(data9)) {
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
                                      if (data10.length === 0 || pattern19.test(data10)) {
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
    validate63.errors = vErrors;
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
  validate63.errors = vErrors;
  return errors === 0;
}
var pattern20 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern21 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern22 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern23 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern24 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern25 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern26 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate62(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.id === void 0 && (missing0 = "id") || data.modelDir === void 0 && (missing0 = "modelDir") || data.displayName === void 0 && (missing0 = "displayName") || data.modelType === void 0 && (missing0 = "modelType") || data.dependencyCount === void 0 && (missing0 = "dependencyCount") || data.relatedAvailable === void 0 && (missing0 = "relatedAvailable") || data.artifact === void 0 && (missing0 = "artifact") || data.integrity === void 0 && (missing0 = "integrity")) {
        validate62.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema67.properties, key0)) {
            validate62.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.artifact !== void 0) {
            const _errs2 = errors;
            if (!validate63(data.artifact, { instancePath: instancePath + "/artifact", parentData: data, parentDataProperty: "artifact", rootData })) {
              vErrors = vErrors === null ? validate63.errors : vErrors.concat(validate63.errors);
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
                validate62.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs3) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 512 || isNaN(data1)) {
                    validate62.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 512 }, message: "must be <= 512" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate62.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate62.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                      return false;
                    } else {
                      if (data2.length === 0 || pattern20.test(data2)) {
                        validate62.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                        return false;
                      } else {
                        if (encodeURIComponent(data2).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                          validate62.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate62.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                        validate62.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                        return false;
                      } else {
                        if (data3.length === 0 || pattern21.test(data3)) {
                          validate62.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data3).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate62.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate62.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                          validate62.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                          return false;
                        } else {
                          if (data4.length === 0 || pattern22.test(data4)) {
                            validate62.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                            return false;
                          } else {
                            if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                              validate62.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate62.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                            validate62.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                            return false;
                          } else {
                            if (data5.length === 0 || pattern23.test(data5)) {
                              validate62.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                              return false;
                            } else {
                              if (encodeURIComponent(data5).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                validate62.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate62.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                          validate62.errors = vErrors;
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
                                validate62.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                return false;
                              } else {
                                if (data12.length === 0 || pattern24.test(data12)) {
                                  validate62.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                  return false;
                                } else {
                                  if (encodeURIComponent(data12).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                    validate62.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                    return false;
                                  }
                                }
                              }
                            } else {
                              validate62.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                  validate62.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                  return false;
                                } else {
                                  if (data13.length === 0 || pattern25.test(data13)) {
                                    validate62.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                    return false;
                                  } else {
                                    if (encodeURIComponent(data13).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                      validate62.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                      return false;
                                    }
                                  }
                                }
                              } else {
                                validate62.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                    validate62.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                    return false;
                                  } else {
                                    if (data14.length === 0 || pattern26.test(data14)) {
                                      validate62.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                      return false;
                                    } else {
                                      if (encodeURIComponent(data14).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        validate62.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                        return false;
                                      }
                                    }
                                  }
                                } else {
                                  validate62.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                  validate62.errors = [{ instancePath: instancePath + "/relatedAvailable", schemaPath: "#/properties/relatedAvailable/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
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
                                    validate62.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                    return false;
                                  }
                                  if (errors === _errs40) {
                                    if (typeof data16 == "number" && isFinite(data16)) {
                                      if (data16 > 9007199254740991 || isNaN(data16)) {
                                        validate62.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                        return false;
                                      } else {
                                        if (data16 < 0 || isNaN(data16)) {
                                          validate62.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate62.errors = [{ instancePath, schemaPath: "#/pumasCatalogRow", keyword: "pumasCatalogRow", params: {}, message: 'must pass "pumasCatalogRow" keyword validation' }];
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
      validate62.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate62.errors = vErrors;
  return errors === 0;
}
function validate61(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.models === void 0 && (missing0 = "models")) {
        validate61.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "models" || key0 === "success")) {
            validate61.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate62(data0[key1], { instancePath: instancePath + "/models/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data0, parentDataProperty: key1, rootData })) {
                    vErrors = vErrors === null ? validate62.errors : vErrors.concat(validate62.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs5 === errors;
                  if (!valid1) {
                    break;
                  }
                }
                if (_errs4 === errors) {
                  if (Object.entries(data0).some(([key, value]) => value === null || typeof value !== "object" || key !== value.id)) {
                    validate61.errors = [{ instancePath: instancePath + "/models", schemaPath: "#/properties/models/pumasCatalogMap", keyword: "pumasCatalogMap", params: {}, message: 'must pass "pumasCatalogMap" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate61.errors = [{ instancePath: instancePath + "/models", schemaPath: "#/properties/models/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
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
                validate61.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate61.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate61.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate61.errors = vErrors;
  return errors === 0;
}
var validatePartialDownloadOutcome = validate66;
var schema72 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "definitions": { "DownloadStatus": { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" }, "PartialDownloadActionName": { "enum": ["resume", "recover", "attach", "none"], "type": "string" }, "PartialDownloadReason": { "enum": ["hf_client_unavailable", "download_root_busy", "model_not_found", "model_not_partial", "recovery_unavailable", "recovery_context_stale", "resume_rejected", "already_completed", "already_cancelled", "invalid_repo_id", "repo_not_found", "rate_limited", "permission_denied", "network_error", "recover_failed"], "type": "string" } }, "properties": { "action": { "$ref": "#/definitions/PartialDownloadActionName" }, "download_id": { "type": ["string", "null"] }, "error": { "type": ["string", "null"] }, "reason_code": { "anyOf": [{ "$ref": "#/definitions/PartialDownloadReason" }, { "type": "null" }] }, "status": { "anyOf": [{ "$ref": "#/definitions/DownloadStatus" }, { "type": "null" }] }, "success": { "type": "boolean" } }, "pumasPartialOutcome": true, "required": ["success", "action", "download_id", "status", "reason_code", "error"], "title": "PartialDownloadOutcome", "type": "object" };
var schema73 = { "enum": ["resume", "recover", "attach", "none"], "type": "string" };
var schema74 = { "enum": ["hf_client_unavailable", "download_root_busy", "model_not_found", "model_not_partial", "recovery_unavailable", "recovery_context_stale", "resume_rejected", "already_completed", "already_cancelled", "invalid_repo_id", "repo_not_found", "rate_limited", "permission_denied", "network_error", "recover_failed"], "type": "string" };
var schema75 = { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" };
function validate66(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.action === void 0 && (missing0 = "action") || data.download_id === void 0 && (missing0 = "download_id") || data.status === void 0 && (missing0 = "status") || data.reason_code === void 0 && (missing0 = "reason_code") || data.error === void 0 && (missing0 = "error")) {
        validate66.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "action" || key0 === "download_id" || key0 === "error" || key0 === "reason_code" || key0 === "status" || key0 === "success")) {
            validate66.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.action !== void 0) {
            let data0 = data.action;
            const _errs2 = errors;
            if (typeof data0 !== "string") {
              validate66.errors = [{ instancePath: instancePath + "/action", schemaPath: "#/definitions/PartialDownloadActionName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            if (!(data0 === "resume" || data0 === "recover" || data0 === "attach" || data0 === "none")) {
              validate66.errors = [{ instancePath: instancePath + "/action", schemaPath: "#/definitions/PartialDownloadActionName/enum", keyword: "enum", params: { allowedValues: schema73.enum }, message: "must be equal to one of the allowed values" }];
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
                validate66.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/type", keyword: "type", params: { type: schema72.properties.download_id.type }, message: "must be string,null" }];
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
                  validate66.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema72.properties.error.type }, message: "must be string,null" }];
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
                    const err1 = { instancePath: instancePath + "/reason_code", schemaPath: "#/definitions/PartialDownloadReason/enum", keyword: "enum", params: { allowedValues: schema74.enum }, message: "must be equal to one of the allowed values" };
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
                    validate66.errors = vErrors;
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
                      const err5 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/enum", keyword: "enum", params: { allowedValues: schema75.enum }, message: "must be equal to one of the allowed values" };
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
                      validate66.errors = vErrors;
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
                        validate66.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                        return false;
                      }
                      var valid0 = _errs23 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (!((data.action === "resume" || data.action === "recover") && data.success === true && typeof data.download_id === "string" && data.download_id.length > 0 && data.status === "queued" && data.reason_code === null && data.error === null || data.action === "attach" && data.success === true && typeof data.download_id === "string" && data.download_id.length > 0 && ["queued", "downloading", "pausing", "cancelling"].includes(data.status) && data.reason_code === null && data.error === null || data.action === "none" && data.success === false && typeof data.error === "string" && (data.download_id === null && data.status === null && !["already_completed", "already_cancelled", "resume_rejected"].includes(data.reason_code) && data.reason_code !== null || typeof data.download_id === "string" && data.download_id.length > 0 && (data.status === "completed" && data.reason_code === "already_completed" || data.status === "cancelled" && data.reason_code === "already_cancelled" || ["paused", "error"].includes(data.status) && data.reason_code === "resume_rejected")))) {
                        validate66.errors = [{ instancePath, schemaPath: "#/pumasPartialOutcome", keyword: "pumasPartialOutcome", params: {}, message: 'must pass "pumasPartialOutcome" keyword validation' }];
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
      validate66.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate66.errors = vErrors;
  return errors === 0;
}
var validatePublicError = validate67;
var schema77 = { "description": "Stable public failure categories shared by RPC transports.", "enum": ["invalid_request", "not_found", "conflict", "cancelled", "unavailable", "operation_failed", "internal"], "type": "string" };
function validate67(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.code === void 0 && (missing0 = "code") || data.class === void 0 && (missing0 = "class") || data.message === void 0 && (missing0 = "message")) {
        validate67.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "class" || key0 === "code" || key0 === "message")) {
            validate67.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.class !== void 0) {
            let data0 = data.class;
            const _errs2 = errors;
            if (typeof data0 !== "string") {
              validate67.errors = [{ instancePath: instancePath + "/class", schemaPath: "#/definitions/PublicErrorClass/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            if (!(data0 === "invalid_request" || data0 === "not_found" || data0 === "conflict" || data0 === "cancelled" || data0 === "unavailable" || data0 === "operation_failed" || data0 === "internal")) {
              validate67.errors = [{ instancePath: instancePath + "/class", schemaPath: "#/definitions/PublicErrorClass/enum", keyword: "enum", params: { allowedValues: schema77.enum }, message: "must be equal to one of the allowed values" }];
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
                validate67.errors = [{ instancePath: instancePath + "/code", schemaPath: "#/properties/code/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs5) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 2147483647 || isNaN(data1)) {
                    validate67.errors = [{ instancePath: instancePath + "/code", schemaPath: "#/properties/code/maximum", keyword: "maximum", params: { comparison: "<=", limit: 2147483647 }, message: "must be <= 2147483647" }];
                    return false;
                  } else {
                    if (data1 < -2147483648 || isNaN(data1)) {
                      validate67.errors = [{ instancePath: instancePath + "/code", schemaPath: "#/properties/code/minimum", keyword: "minimum", params: { comparison: ">=", limit: -2147483648 }, message: "must be >= -2147483648" }];
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
                  validate67.errors = [{ instancePath: instancePath + "/message", schemaPath: "#/properties/message/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
      validate67.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate67.errors = vErrors;
  return errors === 0;
}
var validateRecoverDownloadParams = validate68;
function validate68(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.modelId === void 0 && (missing0 = "modelId") || data.recoveryToken === void 0 && (missing0 = "recoveryToken")) {
        validate68.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "modelId" || key0 === "recoveryToken")) {
            validate68.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  validate68.errors = [{ instancePath: instancePath + "/modelId", schemaPath: "#/properties/modelId/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' }];
                  return false;
                } else {
                  if (encodeURIComponent(data0).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                    validate68.errors = [{ instancePath: instancePath + "/modelId", schemaPath: "#/properties/modelId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate68.errors = [{ instancePath: instancePath + "/modelId", schemaPath: "#/properties/modelId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                    validate68.errors = [{ instancePath: instancePath + "/recoveryToken", schemaPath: "#/properties/recoveryToken/pattern", keyword: "pattern", params: { pattern: "^v1:[0-9a-f]{64}$" }, message: 'must match pattern "^v1:[0-9a-f]{64}$"' }];
                    return false;
                  }
                } else {
                  validate68.errors = [{ instancePath: instancePath + "/recoveryToken", schemaPath: "#/properties/recoveryToken/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
      validate68.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate68.errors = vErrors;
  return errors === 0;
}
var validateSearchCatalogParams = validate69;
var schema79 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "properties": { "limit": { "maximum": 512, "minimum": 1, "type": ["integer", "null"] }, "offset": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "query": { "pumasUtf8Max": 4096, "type": "string" } }, "required": ["query"], "title": "SearchCatalogParams", "type": "object" };
function validate69(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.query === void 0 && (missing0 = "query")) {
        validate69.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "limit" || key0 === "offset" || key0 === "query")) {
            validate69.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.limit !== void 0) {
            let data0 = data.limit;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0)) && data0 !== null) {
              validate69.errors = [{ instancePath: instancePath + "/limit", schemaPath: "#/properties/limit/type", keyword: "type", params: { type: schema79.properties.limit.type }, message: "must be integer,null" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 512 || isNaN(data0)) {
                  validate69.errors = [{ instancePath: instancePath + "/limit", schemaPath: "#/properties/limit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 512 }, message: "must be <= 512" }];
                  return false;
                } else {
                  if (data0 < 1 || isNaN(data0)) {
                    validate69.errors = [{ instancePath: instancePath + "/limit", schemaPath: "#/properties/limit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
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
                validate69.errors = [{ instancePath: instancePath + "/offset", schemaPath: "#/properties/offset/type", keyword: "type", params: { type: schema79.properties.offset.type }, message: "must be integer,null" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 4294967295 || isNaN(data1)) {
                    validate69.errors = [{ instancePath: instancePath + "/offset", schemaPath: "#/properties/offset/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate69.errors = [{ instancePath: instancePath + "/offset", schemaPath: "#/properties/offset/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate69.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                      return false;
                    }
                  } else {
                    validate69.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
      validate69.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate69.errors = vErrors;
  return errors === 0;
}
var validateStartBackendSetupParams = validate70;
var schema80 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "definitions": { "QuantBackend": { "description": "Identifies which quantization backend provides a capability.", "oneOf": [{ "const": "python_conversion", "description": "Existing Python-based safetensors \u2194 GGUF F16 conversion.", "type": "string" }, { "const": "llama_cpp", "description": "llama.cpp native quantization (llama-quantize, llama-imatrix).", "type": "string" }, { "const": "nvfp4", "description": "NVIDIA NVFP4 via TensorRT-LLM / nvidia-modelopt (Phase 2).", "type": "string" }, { "const": "sherry", "description": "Sherry / AngelSlim quantization-aware training (Phase 3).", "type": "string" }] } }, "properties": { "backend": { "$ref": "#/definitions/QuantBackend" }, "expected_previous_operation_id": { "default": null, "maxLength": 36, "minLength": 36, "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "type": ["string", "null"] } }, "required": ["backend"], "title": "StartBackendSetupParams", "type": "object" };
function validate70(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.backend === void 0 && (missing0 = "backend")) {
        validate70.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backend" || key0 === "expected_previous_operation_id")) {
            validate70.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.backend !== void 0) {
            let data0 = data.backend;
            const _errs2 = errors;
            const _errs4 = errors;
            let valid2 = false;
            let passing0 = null;
            const _errs5 = errors;
            if (typeof data0 !== "string") {
              const err0 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err0];
              } else {
                vErrors.push(err0);
              }
              errors++;
            }
            if ("python_conversion" !== data0) {
              const err1 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/0/const", keyword: "const", params: { allowedValue: "python_conversion" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err1];
              } else {
                vErrors.push(err1);
              }
              errors++;
            }
            var _valid0 = _errs5 === errors;
            if (_valid0) {
              valid2 = true;
              passing0 = 0;
            }
            const _errs7 = errors;
            if (typeof data0 !== "string") {
              const err2 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("llama_cpp" !== data0) {
              const err3 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/1/const", keyword: "const", params: { allowedValue: "llama_cpp" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
            var _valid0 = _errs7 === errors;
            if (_valid0 && valid2) {
              valid2 = false;
              passing0 = [passing0, 1];
            } else {
              if (_valid0) {
                valid2 = true;
                passing0 = 1;
              }
              const _errs9 = errors;
              if (typeof data0 !== "string") {
                const err4 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err4];
                } else {
                  vErrors.push(err4);
                }
                errors++;
              }
              if ("nvfp4" !== data0) {
                const err5 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/2/const", keyword: "const", params: { allowedValue: "nvfp4" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err5];
                } else {
                  vErrors.push(err5);
                }
                errors++;
              }
              var _valid0 = _errs9 === errors;
              if (_valid0 && valid2) {
                valid2 = false;
                passing0 = [passing0, 2];
              } else {
                if (_valid0) {
                  valid2 = true;
                  passing0 = 2;
                }
                const _errs11 = errors;
                if (typeof data0 !== "string") {
                  const err6 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err6];
                  } else {
                    vErrors.push(err6);
                  }
                  errors++;
                }
                if ("sherry" !== data0) {
                  const err7 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/3/const", keyword: "const", params: { allowedValue: "sherry" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                }
                var _valid0 = _errs11 === errors;
                if (_valid0 && valid2) {
                  valid2 = false;
                  passing0 = [passing0, 3];
                } else {
                  if (_valid0) {
                    valid2 = true;
                    passing0 = 3;
                  }
                }
              }
            }
            if (!valid2) {
              const err8 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
              if (vErrors === null) {
                vErrors = [err8];
              } else {
                vErrors.push(err8);
              }
              errors++;
              validate70.errors = vErrors;
              return false;
            } else {
              errors = _errs4;
              if (vErrors !== null) {
                if (_errs4) {
                  vErrors.length = _errs4;
                } else {
                  vErrors = null;
                }
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.expected_previous_operation_id !== void 0) {
              let data1 = data.expected_previous_operation_id;
              const _errs13 = errors;
              if (typeof data1 !== "string" && data1 !== null) {
                validate70.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/type", keyword: "type", params: { type: schema80.properties.expected_previous_operation_id.type }, message: "must be string,null" }];
                return false;
              }
              if (errors === _errs13) {
                if (typeof data1 === "string") {
                  if (func4(data1) > 36) {
                    validate70.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/maxLength", keyword: "maxLength", params: { limit: 36 }, message: "must NOT have more than 36 characters" }];
                    return false;
                  } else {
                    if (func4(data1) < 36) {
                      validate70.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/minLength", keyword: "minLength", params: { limit: 36 }, message: "must NOT have fewer than 36 characters" }];
                      return false;
                    } else {
                      if (!pattern12.test(data1)) {
                        validate70.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/pattern", keyword: "pattern", params: { pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$" }, message: 'must match pattern "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"' }];
                        return false;
                      }
                    }
                  }
                }
              }
              var valid0 = _errs13 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate70.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate70.errors = vErrors;
  return errors === 0;
}
var validateStartConversionSetupParams = validate71;
var schema82 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "properties": { "expected_previous_operation_id": { "default": null, "maxLength": 36, "minLength": 36, "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "type": ["string", "null"] } }, "title": "StartConversionSetupParams", "type": "object" };
function validate71(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      const _errs1 = errors;
      for (const key0 in data) {
        if (!(key0 === "expected_previous_operation_id")) {
          validate71.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
          return false;
          break;
        }
      }
      if (_errs1 === errors) {
        if (data.expected_previous_operation_id !== void 0) {
          let data0 = data.expected_previous_operation_id;
          const _errs2 = errors;
          if (typeof data0 !== "string" && data0 !== null) {
            validate71.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/type", keyword: "type", params: { type: schema82.properties.expected_previous_operation_id.type }, message: "must be string,null" }];
            return false;
          }
          if (errors === _errs2) {
            if (typeof data0 === "string") {
              if (func4(data0) > 36) {
                validate71.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/maxLength", keyword: "maxLength", params: { limit: 36 }, message: "must NOT have more than 36 characters" }];
                return false;
              } else {
                if (func4(data0) < 36) {
                  validate71.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/minLength", keyword: "minLength", params: { limit: 36 }, message: "must NOT have fewer than 36 characters" }];
                  return false;
                } else {
                  if (!pattern12.test(data0)) {
                    validate71.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/pattern", keyword: "pattern", params: { pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$" }, message: 'must match pattern "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"' }];
                    return false;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate71.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate71.errors = vErrors;
  return errors === 0;
}
var validateSuccessOutcome = validate72;
function validate72(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success")) {
        validate72.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "success")) {
            validate72.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.success !== void 0) {
            let data0 = data.success;
            if (typeof data0 !== "boolean") {
              validate72.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
            }
            if (true !== data0) {
              validate72.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
              return false;
            }
          }
        }
      }
    } else {
      validate72.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate72.errors = vErrors;
  return errors === 0;
}
var validateSupportedQuantTypesOutcome = validate73;
function validate74(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.name === void 0 && (missing0 = "name") || data.description === void 0 && (missing0 = "description") || data.bitsPerWeight === void 0 && (missing0 = "bitsPerWeight") || data.recommended === void 0 && (missing0 = "recommended") || data.backend === void 0 && (missing0 = "backend") || data.imatrixRecommended === void 0 && (missing0 = "imatrixRecommended")) {
        validate74.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backend" || key0 === "bitsPerWeight" || key0 === "description" || key0 === "imatrixRecommended" || key0 === "name" || key0 === "recommended")) {
            validate74.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.backend !== void 0) {
            let data0 = data.backend;
            const _errs2 = errors;
            const _errs3 = errors;
            let valid1 = false;
            const _errs4 = errors;
            const _errs6 = errors;
            let valid3 = false;
            let passing0 = null;
            const _errs7 = errors;
            if (typeof data0 !== "string") {
              const err0 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err0];
              } else {
                vErrors.push(err0);
              }
              errors++;
            }
            if ("python_conversion" !== data0) {
              const err1 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/0/const", keyword: "const", params: { allowedValue: "python_conversion" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err1];
              } else {
                vErrors.push(err1);
              }
              errors++;
            }
            var _valid1 = _errs7 === errors;
            if (_valid1) {
              valid3 = true;
              passing0 = 0;
            }
            const _errs9 = errors;
            if (typeof data0 !== "string") {
              const err2 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("llama_cpp" !== data0) {
              const err3 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/1/const", keyword: "const", params: { allowedValue: "llama_cpp" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
            var _valid1 = _errs9 === errors;
            if (_valid1 && valid3) {
              valid3 = false;
              passing0 = [passing0, 1];
            } else {
              if (_valid1) {
                valid3 = true;
                passing0 = 1;
              }
              const _errs11 = errors;
              if (typeof data0 !== "string") {
                const err4 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err4];
                } else {
                  vErrors.push(err4);
                }
                errors++;
              }
              if ("nvfp4" !== data0) {
                const err5 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/2/const", keyword: "const", params: { allowedValue: "nvfp4" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err5];
                } else {
                  vErrors.push(err5);
                }
                errors++;
              }
              var _valid1 = _errs11 === errors;
              if (_valid1 && valid3) {
                valid3 = false;
                passing0 = [passing0, 2];
              } else {
                if (_valid1) {
                  valid3 = true;
                  passing0 = 2;
                }
                const _errs13 = errors;
                if (typeof data0 !== "string") {
                  const err6 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err6];
                  } else {
                    vErrors.push(err6);
                  }
                  errors++;
                }
                if ("sherry" !== data0) {
                  const err7 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf/3/const", keyword: "const", params: { allowedValue: "sherry" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                }
                var _valid1 = _errs13 === errors;
                if (_valid1 && valid3) {
                  valid3 = false;
                  passing0 = [passing0, 3];
                } else {
                  if (_valid1) {
                    valid3 = true;
                    passing0 = 3;
                  }
                }
              }
            }
            if (!valid3) {
              const err8 = { instancePath: instancePath + "/backend", schemaPath: "#/definitions/QuantBackend/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
              if (vErrors === null) {
                vErrors = [err8];
              } else {
                vErrors.push(err8);
              }
              errors++;
            } else {
              errors = _errs6;
              if (vErrors !== null) {
                if (_errs6) {
                  vErrors.length = _errs6;
                } else {
                  vErrors = null;
                }
              }
            }
            var _valid0 = _errs4 === errors;
            valid1 = valid1 || _valid0;
            if (!valid1) {
              const _errs15 = errors;
              if (data0 !== null) {
                const err9 = { instancePath: instancePath + "/backend", schemaPath: "#/properties/backend/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
                if (vErrors === null) {
                  vErrors = [err9];
                } else {
                  vErrors.push(err9);
                }
                errors++;
              }
              var _valid0 = _errs15 === errors;
              valid1 = valid1 || _valid0;
            }
            if (!valid1) {
              const err10 = { instancePath: instancePath + "/backend", schemaPath: "#/properties/backend/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
              if (vErrors === null) {
                vErrors = [err10];
              } else {
                vErrors.push(err10);
              }
              errors++;
              validate74.errors = vErrors;
              return false;
            } else {
              errors = _errs3;
              if (vErrors !== null) {
                if (_errs3) {
                  vErrors.length = _errs3;
                } else {
                  vErrors = null;
                }
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.bitsPerWeight !== void 0) {
              let data1 = data.bitsPerWeight;
              const _errs17 = errors;
              if (errors === _errs17) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 34028234663852886e22 || isNaN(data1)) {
                    validate74.errors = [{ instancePath: instancePath + "/bitsPerWeight", schemaPath: "#/properties/bitsPerWeight/maximum", keyword: "maximum", params: { comparison: "<=", limit: 34028234663852886e22 }, message: "must be <= 3.4028234663852886e+38" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate74.errors = [{ instancePath: instancePath + "/bitsPerWeight", schemaPath: "#/properties/bitsPerWeight/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                      return false;
                    }
                  }
                } else {
                  validate74.errors = [{ instancePath: instancePath + "/bitsPerWeight", schemaPath: "#/properties/bitsPerWeight/type", keyword: "type", params: { type: "number" }, message: "must be number" }];
                  return false;
                }
              }
              var valid0 = _errs17 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.description !== void 0) {
                const _errs19 = errors;
                if (typeof data.description !== "string") {
                  validate74.errors = [{ instancePath: instancePath + "/description", schemaPath: "#/properties/description/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                  return false;
                }
                var valid0 = _errs19 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.imatrixRecommended !== void 0) {
                  const _errs21 = errors;
                  if (typeof data.imatrixRecommended !== "boolean") {
                    validate74.errors = [{ instancePath: instancePath + "/imatrixRecommended", schemaPath: "#/properties/imatrixRecommended/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                    return false;
                  }
                  var valid0 = _errs21 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.name !== void 0) {
                    const _errs23 = errors;
                    if (typeof data.name !== "string") {
                      validate74.errors = [{ instancePath: instancePath + "/name", schemaPath: "#/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                      return false;
                    }
                    var valid0 = _errs23 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.recommended !== void 0) {
                      const _errs25 = errors;
                      if (typeof data.recommended !== "boolean") {
                        validate74.errors = [{ instancePath: instancePath + "/recommended", schemaPath: "#/properties/recommended/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                        return false;
                      }
                      var valid0 = _errs25 === errors;
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
    } else {
      validate74.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate74.errors = vErrors;
  return errors === 0;
}
function validate73(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.quant_types === void 0 && (missing0 = "quant_types")) {
        validate73.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "quant_types" || key0 === "success")) {
            validate73.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.quant_types !== void 0) {
            let data0 = data.quant_types;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (!validate74(data0[i0], { instancePath: instancePath + "/quant_types/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate74.errors : vErrors.concat(validate74.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate73.errors = [{ instancePath: instancePath + "/quant_types", schemaPath: "#/properties/quant_types/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate73.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate73.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate73.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate73.errors = vErrors;
  return errors === 0;
}
export {
  validateBackendStatusOutcome,
  validateCatalogSearchOutcome,
  validateConversionCancelledOutcome,
  validateConversionEnvironmentOutcome,
  validateConversionListOutcome,
  validateConversionProgressResponse,
  validateConversionSetupStartedOutcome,
  validateConversionSetupStatusOutcome,
  validateConversionStartedOutcome,
  validateDownloadIdParams,
  validateDownloadListOutcome,
  validateDownloadMutationOutcome,
  validateDownloadStartedOutcome,
  validateDownloadStatusOutcome,
  validateGetBackendSetupParams,
  validateGetHfDownloadDetailsParams,
  validateHfDownloadDetailsOutcome,
  validateInferenceSettingsOutcome,
  validateLinkHealthOutcome,
  validateModelIndexRefreshOutcome,
  validateModelsOutcome,
  validatePartialDownloadOutcome,
  validatePublicError,
  validateRecoverDownloadParams,
  validateSearchCatalogParams,
  validateStartBackendSetupParams,
  validateStartConversionSetupParams,
  validateSuccessOutcome,
  validateSupportedQuantTypesOutcome
};
