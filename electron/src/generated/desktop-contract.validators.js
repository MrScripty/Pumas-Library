// Generated from pumas-rpc contract.rs; SHA256 ca65b9828a4b76f636af0b0654c5539555bd4b73f8b26c779c442571e4d9bf87. DO NOT EDIT.
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
var validateAvailableVersionsOutcome = validate10;
var schema15 = { "additionalProperties": false, "properties": { "error": { "type": "string" }, "rate_limited": { "const": true, "type": "boolean" }, "retry_after_secs": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "success": { "const": false, "type": "boolean" } }, "required": ["success", "error", "rate_limited", "retry_after_secs"], "type": "object" };
var schema13 = { "additionalProperties": false, "description": "Version release info as returned to frontend.", "properties": { "archiveSize": { "default": null, "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "assets": { "default": [], "items": { "$ref": "#/definitions/VersionReleaseAsset" }, "type": "array" }, "body": { "default": null, "type": ["string", "null"] }, "dependenciesSize": { "default": null, "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "htmlUrl": { "type": "string" }, "installing": { "default": null, "type": ["boolean", "null"] }, "name": { "type": "string" }, "prerelease": { "default": false, "type": "boolean" }, "publishedAt": { "type": "string" }, "tagName": { "type": "string" }, "totalSize": { "default": null, "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["tagName", "name", "publishedAt", "prerelease", "body", "htmlUrl", "assets", "totalSize", "archiveSize", "dependenciesSize", "installing"], "type": "object" };
var func2 = Object.prototype.hasOwnProperty;
function validate12(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.tagName === void 0 && (missing0 = "tagName") || data.name === void 0 && (missing0 = "name") || data.publishedAt === void 0 && (missing0 = "publishedAt") || data.prerelease === void 0 && (missing0 = "prerelease") || data.body === void 0 && (missing0 = "body") || data.htmlUrl === void 0 && (missing0 = "htmlUrl") || data.assets === void 0 && (missing0 = "assets") || data.totalSize === void 0 && (missing0 = "totalSize") || data.archiveSize === void 0 && (missing0 = "archiveSize") || data.dependenciesSize === void 0 && (missing0 = "dependenciesSize") || data.installing === void 0 && (missing0 = "installing")) {
        validate12.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema13.properties, key0)) {
            validate12.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.archiveSize !== void 0) {
            let data0 = data.archiveSize;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0)) && data0 !== null) {
              validate12.errors = [{ instancePath: instancePath + "/archiveSize", schemaPath: "#/properties/archiveSize/type", keyword: "type", params: { type: schema13.properties.archiveSize.type }, message: "must be integer,null" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 9007199254740991 || isNaN(data0)) {
                  validate12.errors = [{ instancePath: instancePath + "/archiveSize", schemaPath: "#/properties/archiveSize/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                  return false;
                } else {
                  if (data0 < 0 || isNaN(data0)) {
                    validate12.errors = [{ instancePath: instancePath + "/archiveSize", schemaPath: "#/properties/archiveSize/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
            if (data.assets !== void 0) {
              let data1 = data.assets;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (Array.isArray(data1)) {
                  var valid1 = true;
                  const len0 = data1.length;
                  for (let i0 = 0; i0 < len0; i0++) {
                    let data2 = data1[i0];
                    const _errs6 = errors;
                    const _errs7 = errors;
                    if (errors === _errs7) {
                      if (data2 && typeof data2 == "object" && !Array.isArray(data2)) {
                        let missing1;
                        if (data2.name === void 0 && (missing1 = "name") || data2.size === void 0 && (missing1 = "size") || data2.downloadUrl === void 0 && (missing1 = "downloadUrl")) {
                          validate12.errors = [{ instancePath: instancePath + "/assets/" + i0, schemaPath: "#/definitions/VersionReleaseAsset/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" }];
                          return false;
                        } else {
                          const _errs9 = errors;
                          for (const key1 in data2) {
                            if (!(key1 === "downloadUrl" || key1 === "name" || key1 === "size")) {
                              validate12.errors = [{ instancePath: instancePath + "/assets/" + i0, schemaPath: "#/definitions/VersionReleaseAsset/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" }];
                              return false;
                              break;
                            }
                          }
                          if (_errs9 === errors) {
                            if (data2.downloadUrl !== void 0) {
                              const _errs10 = errors;
                              if (typeof data2.downloadUrl !== "string") {
                                validate12.errors = [{ instancePath: instancePath + "/assets/" + i0 + "/downloadUrl", schemaPath: "#/definitions/VersionReleaseAsset/properties/downloadUrl/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                return false;
                              }
                              var valid3 = _errs10 === errors;
                            } else {
                              var valid3 = true;
                            }
                            if (valid3) {
                              if (data2.name !== void 0) {
                                const _errs12 = errors;
                                if (typeof data2.name !== "string") {
                                  validate12.errors = [{ instancePath: instancePath + "/assets/" + i0 + "/name", schemaPath: "#/definitions/VersionReleaseAsset/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                  return false;
                                }
                                var valid3 = _errs12 === errors;
                              } else {
                                var valid3 = true;
                              }
                              if (valid3) {
                                if (data2.size !== void 0) {
                                  let data5 = data2.size;
                                  const _errs14 = errors;
                                  if (!(typeof data5 == "number" && (!(data5 % 1) && !isNaN(data5)) && isFinite(data5))) {
                                    validate12.errors = [{ instancePath: instancePath + "/assets/" + i0 + "/size", schemaPath: "#/definitions/VersionReleaseAsset/properties/size/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                    return false;
                                  }
                                  if (errors === _errs14) {
                                    if (typeof data5 == "number" && isFinite(data5)) {
                                      if (data5 > 9007199254740991 || isNaN(data5)) {
                                        validate12.errors = [{ instancePath: instancePath + "/assets/" + i0 + "/size", schemaPath: "#/definitions/VersionReleaseAsset/properties/size/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                        return false;
                                      } else {
                                        if (data5 < 0 || isNaN(data5)) {
                                          validate12.errors = [{ instancePath: instancePath + "/assets/" + i0 + "/size", schemaPath: "#/definitions/VersionReleaseAsset/properties/size/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                          return false;
                                        }
                                      }
                                    }
                                  }
                                  var valid3 = _errs14 === errors;
                                } else {
                                  var valid3 = true;
                                }
                              }
                            }
                          }
                        }
                      } else {
                        validate12.errors = [{ instancePath: instancePath + "/assets/" + i0, schemaPath: "#/definitions/VersionReleaseAsset/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                        return false;
                      }
                    }
                    var valid1 = _errs6 === errors;
                    if (!valid1) {
                      break;
                    }
                  }
                } else {
                  validate12.errors = [{ instancePath: instancePath + "/assets", schemaPath: "#/properties/assets/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                  return false;
                }
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.body !== void 0) {
                let data6 = data.body;
                const _errs16 = errors;
                if (typeof data6 !== "string" && data6 !== null) {
                  validate12.errors = [{ instancePath: instancePath + "/body", schemaPath: "#/properties/body/type", keyword: "type", params: { type: schema13.properties.body.type }, message: "must be string,null" }];
                  return false;
                }
                var valid0 = _errs16 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.dependenciesSize !== void 0) {
                  let data7 = data.dependenciesSize;
                  const _errs18 = errors;
                  if (!(typeof data7 == "number" && (!(data7 % 1) && !isNaN(data7)) && isFinite(data7)) && data7 !== null) {
                    validate12.errors = [{ instancePath: instancePath + "/dependenciesSize", schemaPath: "#/properties/dependenciesSize/type", keyword: "type", params: { type: schema13.properties.dependenciesSize.type }, message: "must be integer,null" }];
                    return false;
                  }
                  if (errors === _errs18) {
                    if (typeof data7 == "number" && isFinite(data7)) {
                      if (data7 > 9007199254740991 || isNaN(data7)) {
                        validate12.errors = [{ instancePath: instancePath + "/dependenciesSize", schemaPath: "#/properties/dependenciesSize/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                        return false;
                      } else {
                        if (data7 < 0 || isNaN(data7)) {
                          validate12.errors = [{ instancePath: instancePath + "/dependenciesSize", schemaPath: "#/properties/dependenciesSize/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                  if (data.htmlUrl !== void 0) {
                    const _errs20 = errors;
                    if (typeof data.htmlUrl !== "string") {
                      validate12.errors = [{ instancePath: instancePath + "/htmlUrl", schemaPath: "#/properties/htmlUrl/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                      return false;
                    }
                    var valid0 = _errs20 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.installing !== void 0) {
                      let data9 = data.installing;
                      const _errs22 = errors;
                      if (typeof data9 !== "boolean" && data9 !== null) {
                        validate12.errors = [{ instancePath: instancePath + "/installing", schemaPath: "#/properties/installing/type", keyword: "type", params: { type: schema13.properties.installing.type }, message: "must be boolean,null" }];
                        return false;
                      }
                      var valid0 = _errs22 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.name !== void 0) {
                        const _errs24 = errors;
                        if (typeof data.name !== "string") {
                          validate12.errors = [{ instancePath: instancePath + "/name", schemaPath: "#/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                          return false;
                        }
                        var valid0 = _errs24 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.prerelease !== void 0) {
                          const _errs26 = errors;
                          if (typeof data.prerelease !== "boolean") {
                            validate12.errors = [{ instancePath: instancePath + "/prerelease", schemaPath: "#/properties/prerelease/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                            return false;
                          }
                          var valid0 = _errs26 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.publishedAt !== void 0) {
                            const _errs28 = errors;
                            if (typeof data.publishedAt !== "string") {
                              validate12.errors = [{ instancePath: instancePath + "/publishedAt", schemaPath: "#/properties/publishedAt/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                              return false;
                            }
                            var valid0 = _errs28 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.tagName !== void 0) {
                              const _errs30 = errors;
                              if (typeof data.tagName !== "string") {
                                validate12.errors = [{ instancePath: instancePath + "/tagName", schemaPath: "#/properties/tagName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                return false;
                              }
                              var valid0 = _errs30 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.totalSize !== void 0) {
                                let data14 = data.totalSize;
                                const _errs32 = errors;
                                if (!(typeof data14 == "number" && (!(data14 % 1) && !isNaN(data14)) && isFinite(data14)) && data14 !== null) {
                                  validate12.errors = [{ instancePath: instancePath + "/totalSize", schemaPath: "#/properties/totalSize/type", keyword: "type", params: { type: schema13.properties.totalSize.type }, message: "must be integer,null" }];
                                  return false;
                                }
                                if (errors === _errs32) {
                                  if (typeof data14 == "number" && isFinite(data14)) {
                                    if (data14 > 9007199254740991 || isNaN(data14)) {
                                      validate12.errors = [{ instancePath: instancePath + "/totalSize", schemaPath: "#/properties/totalSize/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                      return false;
                                    } else {
                                      if (data14 < 0 || isNaN(data14)) {
                                        validate12.errors = [{ instancePath: instancePath + "/totalSize", schemaPath: "#/properties/totalSize/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                        return false;
                                      }
                                    }
                                  }
                                }
                                var valid0 = _errs32 === errors;
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
    } else {
      validate12.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate12.errors = vErrors;
  return errors === 0;
}
function validate11(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.versions === void 0 && (missing0 = "versions")) {
        validate11.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "success" || key0 === "versions")) {
            validate11.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.success !== void 0) {
            let data0 = data.success;
            const _errs2 = errors;
            if (typeof data0 !== "boolean") {
              validate11.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
            }
            if (true !== data0) {
              validate11.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.versions !== void 0) {
              let data1 = data.versions;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (Array.isArray(data1)) {
                  var valid1 = true;
                  const len0 = data1.length;
                  for (let i0 = 0; i0 < len0; i0++) {
                    const _errs6 = errors;
                    if (!validate12(data1[i0], { instancePath: instancePath + "/versions/" + i0, parentData: data1, parentDataProperty: i0, rootData })) {
                      vErrors = vErrors === null ? validate12.errors : vErrors.concat(validate12.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs6 === errors;
                    if (!valid1) {
                      break;
                    }
                  }
                } else {
                  validate11.errors = [{ instancePath: instancePath + "/versions", schemaPath: "#/properties/versions/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (!validate11(data, { instancePath, parentData, parentDataProperty, rootData })) {
    vErrors = vErrors === null ? validate11.errors : vErrors.concat(validate11.errors);
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
        if (data.success === void 0 && (missing0 = "success") || data.error === void 0 && (missing0 = "error") || data.rate_limited === void 0 && (missing0 = "rate_limited") || data.retry_after_secs === void 0 && (missing0 = "retry_after_secs")) {
          const err0 = { instancePath, schemaPath: "#/definitions/AvailableVersionsRateLimited/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
          if (vErrors === null) {
            vErrors = [err0];
          } else {
            vErrors.push(err0);
          }
          errors++;
        } else {
          const _errs5 = errors;
          for (const key0 in data) {
            if (!(key0 === "error" || key0 === "rate_limited" || key0 === "retry_after_secs" || key0 === "success")) {
              const err1 = { instancePath, schemaPath: "#/definitions/AvailableVersionsRateLimited/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
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
                const err2 = { instancePath: instancePath + "/error", schemaPath: "#/definitions/AvailableVersionsRateLimited/properties/error/type", keyword: "type", params: { type: "string" }, message: "must be string" };
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
              if (data.rate_limited !== void 0) {
                let data1 = data.rate_limited;
                const _errs8 = errors;
                if (typeof data1 !== "boolean") {
                  const err3 = { instancePath: instancePath + "/rate_limited", schemaPath: "#/definitions/AvailableVersionsRateLimited/properties/rate_limited/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                  if (vErrors === null) {
                    vErrors = [err3];
                  } else {
                    vErrors.push(err3);
                  }
                  errors++;
                }
                if (true !== data1) {
                  const err4 = { instancePath: instancePath + "/rate_limited", schemaPath: "#/definitions/AvailableVersionsRateLimited/properties/rate_limited/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" };
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
              if (valid2) {
                if (data.retry_after_secs !== void 0) {
                  let data2 = data.retry_after_secs;
                  const _errs10 = errors;
                  if (!(typeof data2 == "number" && (!(data2 % 1) && !isNaN(data2)) && isFinite(data2)) && data2 !== null) {
                    const err5 = { instancePath: instancePath + "/retry_after_secs", schemaPath: "#/definitions/AvailableVersionsRateLimited/properties/retry_after_secs/type", keyword: "type", params: { type: schema15.properties.retry_after_secs.type }, message: "must be integer,null" };
                    if (vErrors === null) {
                      vErrors = [err5];
                    } else {
                      vErrors.push(err5);
                    }
                    errors++;
                  }
                  if (errors === _errs10) {
                    if (typeof data2 == "number" && isFinite(data2)) {
                      if (data2 > 9007199254740991 || isNaN(data2)) {
                        const err6 = { instancePath: instancePath + "/retry_after_secs", schemaPath: "#/definitions/AvailableVersionsRateLimited/properties/retry_after_secs/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" };
                        if (vErrors === null) {
                          vErrors = [err6];
                        } else {
                          vErrors.push(err6);
                        }
                        errors++;
                      } else {
                        if (data2 < 0 || isNaN(data2)) {
                          const err7 = { instancePath: instancePath + "/retry_after_secs", schemaPath: "#/definitions/AvailableVersionsRateLimited/properties/retry_after_secs/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
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
                  var valid2 = _errs10 === errors;
                } else {
                  var valid2 = true;
                }
                if (valid2) {
                  if (data.success !== void 0) {
                    let data3 = data.success;
                    const _errs12 = errors;
                    if (typeof data3 !== "boolean") {
                      const err8 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/AvailableVersionsRateLimited/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                      if (vErrors === null) {
                        vErrors = [err8];
                      } else {
                        vErrors.push(err8);
                      }
                      errors++;
                    }
                    if (false !== data3) {
                      const err9 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/AvailableVersionsRateLimited/properties/success/const", keyword: "const", params: { allowedValue: false }, message: "must be equal to constant" };
                      if (vErrors === null) {
                        vErrors = [err9];
                      } else {
                        vErrors.push(err9);
                      }
                      errors++;
                    }
                    var valid2 = _errs12 === errors;
                  } else {
                    var valid2 = true;
                  }
                }
              }
            }
          }
        }
      } else {
        const err10 = { instancePath, schemaPath: "#/definitions/AvailableVersionsRateLimited/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
    var _valid0 = _errs2 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err11 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
    validate10.errors = vErrors;
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
  validate10.errors = vErrors;
  return errors === 0;
}
var validateBackendStatusOutcome = validate15;
function validate16(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.backend === void 0 && (missing0 = "backend") || data.name === void 0 && (missing0 = "name") || data.ready === void 0 && (missing0 = "ready")) {
        validate16.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backend" || key0 === "name" || key0 === "ready")) {
            validate16.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
              validate16.errors = vErrors;
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
                validate16.errors = [{ instancePath: instancePath + "/name", schemaPath: "#/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                  validate16.errors = [{ instancePath: instancePath + "/ready", schemaPath: "#/properties/ready/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
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
      validate16.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate16.errors = vErrors;
  return errors === 0;
}
function validate15(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.backends === void 0 && (missing0 = "backends")) {
        validate15.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backends" || key0 === "success")) {
            validate15.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate16(data0[i0], { instancePath: instancePath + "/backends/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate16.errors : vErrors.concat(validate16.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate15.errors = [{ instancePath: instancePath + "/backends", schemaPath: "#/properties/backends/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate15.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate15.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate15.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate15.errors = vErrors;
  return errors === 0;
}
var validateCancelInstallationOutcome = validate18;
function validate18(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success")) {
        validate18.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "success")) {
            validate18.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.success !== void 0) {
            if (typeof data.success !== "boolean") {
              validate18.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
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
var validateCatalogSearchOutcome = validate19;
var schema21 = { "additionalProperties": false, "properties": { "artifact": { "$ref": "#/definitions/CatalogArtifactState" }, "dependencyCount": { "maximum": 512, "minimum": 0, "type": "integer" }, "displayDate": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "displayName": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "format": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "id": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "integrity": { "$ref": "#/definitions/CatalogIntegrityState" }, "modelDir": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "modelType": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "quantization": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "relatedAvailable": { "type": "boolean" }, "sizeBytes": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" } }, "pumasCatalogRow": true, "required": ["id", "modelDir", "displayName", "modelType", "dependencyCount", "relatedAvailable", "artifact", "integrity"], "type": "object" };
var func5 = require_ucs2length().default;
var schema23 = { "enum": ["part_file_present", "expected_files_missing"], "type": "string" };
var func0 = require_equal().default;
var pattern0 = new RegExp("^v1:[0-9a-f]{64}$", "u");
var pattern1 = new RegExp("^(?!.*(?:--|\\.\\.))(?!.*\\.[gG][iI][tT]$)[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?/[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?$", "u");
var pattern2 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern3 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern4 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate21(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
                          const err14 = { instancePath: instancePath + "/reasons/" + i0, schemaPath: "#/definitions/CatalogPartialReason/enum", keyword: "enum", params: { allowedValues: schema23.enum }, message: "must be equal to one of the allowed values" };
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
                                if (func5(data6) > 96) {
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
var pattern5 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern6 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern7 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern8 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern9 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern10 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern11 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate20(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.id === void 0 && (missing0 = "id") || data.modelDir === void 0 && (missing0 = "modelDir") || data.displayName === void 0 && (missing0 = "displayName") || data.modelType === void 0 && (missing0 = "modelType") || data.dependencyCount === void 0 && (missing0 = "dependencyCount") || data.relatedAvailable === void 0 && (missing0 = "relatedAvailable") || data.artifact === void 0 && (missing0 = "artifact") || data.integrity === void 0 && (missing0 = "integrity")) {
        validate20.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema21.properties, key0)) {
            validate20.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.artifact !== void 0) {
            const _errs2 = errors;
            if (!validate21(data.artifact, { instancePath: instancePath + "/artifact", parentData: data, parentDataProperty: "artifact", rootData })) {
              vErrors = vErrors === null ? validate21.errors : vErrors.concat(validate21.errors);
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
                validate20.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs3) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 512 || isNaN(data1)) {
                    validate20.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 512 }, message: "must be <= 512" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate20.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                    if (func5(data2) < 1) {
                      validate20.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                      return false;
                    } else {
                      if (data2.length === 0 || pattern5.test(data2)) {
                        validate20.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                        return false;
                      } else {
                        if (encodeURIComponent(data2).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                          validate20.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate20.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                      if (func5(data3) < 1) {
                        validate20.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                        return false;
                      } else {
                        if (data3.length === 0 || pattern6.test(data3)) {
                          validate20.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data3).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate20.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate20.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                        if (func5(data4) < 1) {
                          validate20.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                          return false;
                        } else {
                          if (data4.length === 0 || pattern7.test(data4)) {
                            validate20.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                            return false;
                          } else {
                            if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                              validate20.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate20.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                          if (func5(data5) < 1) {
                            validate20.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                            return false;
                          } else {
                            if (data5.length === 0 || pattern8.test(data5)) {
                              validate20.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                              return false;
                            } else {
                              if (encodeURIComponent(data5).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                validate20.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate20.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                          validate20.errors = vErrors;
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
                              if (func5(data12) < 1) {
                                validate20.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                return false;
                              } else {
                                if (data12.length === 0 || pattern9.test(data12)) {
                                  validate20.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                  return false;
                                } else {
                                  if (encodeURIComponent(data12).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                    validate20.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                    return false;
                                  }
                                }
                              }
                            } else {
                              validate20.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                if (func5(data13) < 1) {
                                  validate20.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                  return false;
                                } else {
                                  if (data13.length === 0 || pattern10.test(data13)) {
                                    validate20.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                    return false;
                                  } else {
                                    if (encodeURIComponent(data13).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                      validate20.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                      return false;
                                    }
                                  }
                                }
                              } else {
                                validate20.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                  if (func5(data14) < 1) {
                                    validate20.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                    return false;
                                  } else {
                                    if (data14.length === 0 || pattern11.test(data14)) {
                                      validate20.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                      return false;
                                    } else {
                                      if (encodeURIComponent(data14).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        validate20.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                        return false;
                                      }
                                    }
                                  }
                                } else {
                                  validate20.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                  validate20.errors = [{ instancePath: instancePath + "/relatedAvailable", schemaPath: "#/properties/relatedAvailable/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
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
                                    validate20.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                    return false;
                                  }
                                  if (errors === _errs40) {
                                    if (typeof data16 == "number" && isFinite(data16)) {
                                      if (data16 > 9007199254740991 || isNaN(data16)) {
                                        validate20.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                        return false;
                                      } else {
                                        if (data16 < 0 || isNaN(data16)) {
                                          validate20.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate20.errors = [{ instancePath, schemaPath: "#/pumasCatalogRow", keyword: "pumasCatalogRow", params: {}, message: 'must pass "pumasCatalogRow" keyword validation' }];
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
      validate20.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate20.errors = vErrors;
  return errors === 0;
}
function validate19(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.models === void 0 && (missing0 = "models") || data.total_count === void 0 && (missing0 = "total_count") || data.query_time_ms === void 0 && (missing0 = "query_time_ms") || data.query === void 0 && (missing0 = "query")) {
        validate19.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "models" || key0 === "query" || key0 === "query_time_ms" || key0 === "success" || key0 === "total_count")) {
            validate19.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate20(data0[i0], { instancePath: instancePath + "/models/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate20.errors : vErrors.concat(validate20.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate19.errors = [{ instancePath: instancePath + "/models", schemaPath: "#/properties/models/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                    validate19.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                } else {
                  validate19.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                      validate19.errors = [{ instancePath: instancePath + "/query_time_ms", schemaPath: "#/properties/query_time_ms/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                      return false;
                    } else {
                      if (data3 < 0 || isNaN(data3)) {
                        validate19.errors = [{ instancePath: instancePath + "/query_time_ms", schemaPath: "#/properties/query_time_ms/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                        return false;
                      }
                    }
                  } else {
                    validate19.errors = [{ instancePath: instancePath + "/query_time_ms", schemaPath: "#/properties/query_time_ms/type", keyword: "type", params: { type: "number" }, message: "must be number" }];
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
                    validate19.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                    return false;
                  }
                  if (true !== data4) {
                    validate19.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
                      validate19.errors = [{ instancePath: instancePath + "/total_count", schemaPath: "#/properties/total_count/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                      return false;
                    }
                    if (errors === _errs11) {
                      if (typeof data5 == "number" && isFinite(data5)) {
                        if (data5 > 9007199254740991 || isNaN(data5)) {
                          validate19.errors = [{ instancePath: instancePath + "/total_count", schemaPath: "#/properties/total_count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                          return false;
                        } else {
                          if (data5 < 0 || isNaN(data5)) {
                            validate19.errors = [{ instancePath: instancePath + "/total_count", schemaPath: "#/properties/total_count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate19.errors = [{ instancePath, schemaPath: "#/pumasCatalogSearch", keyword: "pumasCatalogSearch", params: {}, message: 'must pass "pumasCatalogSearch" keyword validation' }];
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
      validate19.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate19.errors = vErrors;
  return errors === 0;
}
var validateCheckVersionDependenciesOutcome = validate24;
var schema27 = { "additionalProperties": false, "properties": { "installed": { "items": { "type": "string" }, "type": "array" }, "missing": { "items": { "type": "string" }, "type": "array" }, "requirementsFile": { "type": ["string", "null"] } }, "required": ["installed", "missing", "requirementsFile"], "type": "object" };
function validate24(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.dependencies === void 0 && (missing0 = "dependencies")) {
        validate24.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "dependencies" || key0 === "success")) {
            validate24.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.dependencies !== void 0) {
            let data0 = data.dependencies;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (data0 && typeof data0 == "object" && !Array.isArray(data0)) {
                let missing1;
                if (data0.installed === void 0 && (missing1 = "installed") || data0.missing === void 0 && (missing1 = "missing") || data0.requirementsFile === void 0 && (missing1 = "requirementsFile")) {
                  validate24.errors = [{ instancePath: instancePath + "/dependencies", schemaPath: "#/definitions/CheckedVersionDependencies/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" }];
                  return false;
                } else {
                  const _errs5 = errors;
                  for (const key1 in data0) {
                    if (!(key1 === "installed" || key1 === "missing" || key1 === "requirementsFile")) {
                      validate24.errors = [{ instancePath: instancePath + "/dependencies", schemaPath: "#/definitions/CheckedVersionDependencies/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" }];
                      return false;
                      break;
                    }
                  }
                  if (_errs5 === errors) {
                    if (data0.installed !== void 0) {
                      let data1 = data0.installed;
                      const _errs6 = errors;
                      if (errors === _errs6) {
                        if (Array.isArray(data1)) {
                          var valid3 = true;
                          const len0 = data1.length;
                          for (let i0 = 0; i0 < len0; i0++) {
                            const _errs8 = errors;
                            if (typeof data1[i0] !== "string") {
                              validate24.errors = [{ instancePath: instancePath + "/dependencies/installed/" + i0, schemaPath: "#/definitions/CheckedVersionDependencies/properties/installed/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                              return false;
                            }
                            var valid3 = _errs8 === errors;
                            if (!valid3) {
                              break;
                            }
                          }
                        } else {
                          validate24.errors = [{ instancePath: instancePath + "/dependencies/installed", schemaPath: "#/definitions/CheckedVersionDependencies/properties/installed/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                          return false;
                        }
                      }
                      var valid2 = _errs6 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data0.missing !== void 0) {
                        let data3 = data0.missing;
                        const _errs10 = errors;
                        if (errors === _errs10) {
                          if (Array.isArray(data3)) {
                            var valid4 = true;
                            const len1 = data3.length;
                            for (let i1 = 0; i1 < len1; i1++) {
                              const _errs12 = errors;
                              if (typeof data3[i1] !== "string") {
                                validate24.errors = [{ instancePath: instancePath + "/dependencies/missing/" + i1, schemaPath: "#/definitions/CheckedVersionDependencies/properties/missing/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                return false;
                              }
                              var valid4 = _errs12 === errors;
                              if (!valid4) {
                                break;
                              }
                            }
                          } else {
                            validate24.errors = [{ instancePath: instancePath + "/dependencies/missing", schemaPath: "#/definitions/CheckedVersionDependencies/properties/missing/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                            return false;
                          }
                        }
                        var valid2 = _errs10 === errors;
                      } else {
                        var valid2 = true;
                      }
                      if (valid2) {
                        if (data0.requirementsFile !== void 0) {
                          let data5 = data0.requirementsFile;
                          const _errs14 = errors;
                          if (typeof data5 !== "string" && data5 !== null) {
                            validate24.errors = [{ instancePath: instancePath + "/dependencies/requirementsFile", schemaPath: "#/definitions/CheckedVersionDependencies/properties/requirementsFile/type", keyword: "type", params: { type: schema27.properties.requirementsFile.type }, message: "must be string,null" }];
                            return false;
                          }
                          var valid2 = _errs14 === errors;
                        } else {
                          var valid2 = true;
                        }
                      }
                    }
                  }
                }
              } else {
                validate24.errors = [{ instancePath: instancePath + "/dependencies", schemaPath: "#/definitions/CheckedVersionDependencies/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.success !== void 0) {
              let data6 = data.success;
              const _errs16 = errors;
              if (typeof data6 !== "boolean") {
                validate24.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data6) {
                validate24.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                return false;
              }
              var valid0 = _errs16 === errors;
            } else {
              var valid0 = true;
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
var validateCheckVersionDependenciesParams = validate25;
function validate25(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.app_id === void 0 && (missing0 = "app_id") || data.tag === void 0 && (missing0 = "tag")) {
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
          if (!(key0 === "app_id" || key0 === "tag")) {
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
          if (data.app_id !== void 0) {
            const _errs4 = errors;
            if (typeof data.app_id !== "string") {
              const err2 = { instancePath: instancePath + "/app_id", schemaPath: "#/anyOf/0/properties/app_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.tag !== void 0) {
              const _errs6 = errors;
              if (typeof data.tag !== "string") {
                const err3 = { instancePath: instancePath + "/tag", schemaPath: "#/anyOf/0/properties/tag/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err3];
                } else {
                  vErrors.push(err3);
                }
                errors++;
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      const err4 = { instancePath, schemaPath: "#/anyOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs8 = errors;
    if (errors === _errs8) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.appId === void 0 && (missing1 = "appId") || data.tag === void 0 && (missing1 = "tag")) {
          const err5 = { instancePath, schemaPath: "#/anyOf/1/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        } else {
          const _errs10 = errors;
          for (const key1 in data) {
            if (!(key1 === "appId" || key1 === "tag")) {
              const err6 = { instancePath, schemaPath: "#/anyOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err6];
              } else {
                vErrors.push(err6);
              }
              errors++;
              break;
            }
          }
          if (_errs10 === errors) {
            if (data.appId !== void 0) {
              const _errs11 = errors;
              if (typeof data.appId !== "string") {
                const err7 = { instancePath: instancePath + "/appId", schemaPath: "#/anyOf/1/properties/appId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err7];
                } else {
                  vErrors.push(err7);
                }
                errors++;
              }
              var valid2 = _errs11 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.tag !== void 0) {
                const _errs13 = errors;
                if (typeof data.tag !== "string") {
                  const err8 = { instancePath: instancePath + "/tag", schemaPath: "#/anyOf/1/properties/tag/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err8];
                  } else {
                    vErrors.push(err8);
                  }
                  errors++;
                }
                var valid2 = _errs13 === errors;
              } else {
                var valid2 = true;
              }
            }
          }
        }
      } else {
        const err9 = { instancePath, schemaPath: "#/anyOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    var _valid0 = _errs8 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
    validate25.errors = vErrors;
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
  validate25.errors = vErrors;
  return errors === 0;
}
var validateConversionCancelledOutcome = validate26;
function validate26(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.cancelled === void 0 && (missing0 = "cancelled")) {
        validate26.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "cancelled" || key0 === "success")) {
            validate26.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.cancelled !== void 0) {
            const _errs2 = errors;
            if (typeof data.cancelled !== "boolean") {
              validate26.errors = [{ instancePath: instancePath + "/cancelled", schemaPath: "#/properties/cancelled/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
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
                validate26.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate26.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate26.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate26.errors = vErrors;
  return errors === 0;
}
var validateConversionEnvironmentOutcome = validate27;
function validate27(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.ready === void 0 && (missing0 = "ready")) {
        validate27.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "ready" || key0 === "success")) {
            validate27.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.ready !== void 0) {
            const _errs2 = errors;
            if (typeof data.ready !== "boolean") {
              validate27.errors = [{ instancePath: instancePath + "/ready", schemaPath: "#/properties/ready/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
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
                validate27.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate27.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate27.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate27.errors = vErrors;
  return errors === 0;
}
var validateConversionListOutcome = validate28;
var schema32 = { "additionalProperties": false, "properties": { "bytesWritten": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "conversionId": { "type": "string" }, "currentTensor": { "type": ["string", "null"] }, "direction": { "$ref": "#/definitions/ConversionDirection" }, "error": { "enum": [null, "The model conversion did not complete successfully."], "type": ["string", "null"] }, "estimatedOutputSize": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "outputModelId": { "type": ["string", "null"] }, "pipelineStep": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "pipelineStepLabel": { "type": ["string", "null"] }, "pipelineStepsTotal": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "sourceModelId": { "type": "string" }, "status": { "$ref": "#/definitions/ConversionStatus" }, "targetQuant": { "type": ["string", "null"] }, "tensorsCompleted": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "tensorsTotal": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] } }, "required": ["conversionId", "sourceModelId", "direction", "status", "progress", "currentTensor", "tensorsCompleted", "tensorsTotal", "bytesWritten", "estimatedOutputSize", "targetQuant", "error", "outputModelId", "pipelineStep", "pipelineStepsTotal", "pipelineStepLabel"], "type": "object" };
function validate29(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.conversionId === void 0 && (missing0 = "conversionId") || data.sourceModelId === void 0 && (missing0 = "sourceModelId") || data.direction === void 0 && (missing0 = "direction") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.currentTensor === void 0 && (missing0 = "currentTensor") || data.tensorsCompleted === void 0 && (missing0 = "tensorsCompleted") || data.tensorsTotal === void 0 && (missing0 = "tensorsTotal") || data.bytesWritten === void 0 && (missing0 = "bytesWritten") || data.estimatedOutputSize === void 0 && (missing0 = "estimatedOutputSize") || data.targetQuant === void 0 && (missing0 = "targetQuant") || data.error === void 0 && (missing0 = "error") || data.outputModelId === void 0 && (missing0 = "outputModelId") || data.pipelineStep === void 0 && (missing0 = "pipelineStep") || data.pipelineStepsTotal === void 0 && (missing0 = "pipelineStepsTotal") || data.pipelineStepLabel === void 0 && (missing0 = "pipelineStepLabel")) {
        validate29.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema32.properties, key0)) {
            validate29.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.bytesWritten !== void 0) {
            let data0 = data.bytesWritten;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0)) && data0 !== null) {
              validate29.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/type", keyword: "type", params: { type: schema32.properties.bytesWritten.type }, message: "must be integer,null" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 9007199254740991 || isNaN(data0)) {
                  validate29.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                  return false;
                } else {
                  if (data0 < 0 || isNaN(data0)) {
                    validate29.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                validate29.errors = [{ instancePath: instancePath + "/conversionId", schemaPath: "#/properties/conversionId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                  validate29.errors = [{ instancePath: instancePath + "/currentTensor", schemaPath: "#/properties/currentTensor/type", keyword: "type", params: { type: schema32.properties.currentTensor.type }, message: "must be string,null" }];
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
                    validate29.errors = vErrors;
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
                      validate29.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema32.properties.error.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (!(data4 === null || data4 === "The model conversion did not complete successfully.")) {
                      validate29.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/enum", keyword: "enum", params: { allowedValues: schema32.properties.error.enum }, message: "must be equal to one of the allowed values" }];
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
                        validate29.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/type", keyword: "type", params: { type: schema32.properties.estimatedOutputSize.type }, message: "must be integer,null" }];
                        return false;
                      }
                      if (errors === _errs25) {
                        if (typeof data5 == "number" && isFinite(data5)) {
                          if (data5 > 9007199254740991 || isNaN(data5)) {
                            validate29.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                            return false;
                          } else {
                            if (data5 < 0 || isNaN(data5)) {
                              validate29.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                          validate29.errors = [{ instancePath: instancePath + "/outputModelId", schemaPath: "#/properties/outputModelId/type", keyword: "type", params: { type: schema32.properties.outputModelId.type }, message: "must be string,null" }];
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
                            validate29.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/type", keyword: "type", params: { type: schema32.properties.pipelineStep.type }, message: "must be integer,null" }];
                            return false;
                          }
                          if (errors === _errs29) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 4294967295 || isNaN(data7)) {
                                validate29.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate29.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                              validate29.errors = [{ instancePath: instancePath + "/pipelineStepLabel", schemaPath: "#/properties/pipelineStepLabel/type", keyword: "type", params: { type: schema32.properties.pipelineStepLabel.type }, message: "must be string,null" }];
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
                                validate29.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/type", keyword: "type", params: { type: schema32.properties.pipelineStepsTotal.type }, message: "must be integer,null" }];
                                return false;
                              }
                              if (errors === _errs33) {
                                if (typeof data9 == "number" && isFinite(data9)) {
                                  if (data9 > 4294967295 || isNaN(data9)) {
                                    validate29.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                    return false;
                                  } else {
                                    if (data9 < 0 || isNaN(data9)) {
                                      validate29.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                  validate29.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema32.properties.progress.type }, message: "must be number,null" }];
                                  return false;
                                }
                                if (errors === _errs35) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 1 || isNaN(data10)) {
                                      validate29.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate29.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate29.errors = [{ instancePath: instancePath + "/sourceModelId", schemaPath: "#/properties/sourceModelId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                      validate29.errors = vErrors;
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
                                        validate29.errors = [{ instancePath: instancePath + "/targetQuant", schemaPath: "#/properties/targetQuant/type", keyword: "type", params: { type: schema32.properties.targetQuant.type }, message: "must be string,null" }];
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
                                          validate29.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/type", keyword: "type", params: { type: schema32.properties.tensorsCompleted.type }, message: "must be integer,null" }];
                                          return false;
                                        }
                                        if (errors === _errs72) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 4294967295 || isNaN(data14)) {
                                              validate29.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate29.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                            validate29.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/type", keyword: "type", params: { type: schema32.properties.tensorsTotal.type }, message: "must be integer,null" }];
                                            return false;
                                          }
                                          if (errors === _errs74) {
                                            if (typeof data15 == "number" && isFinite(data15)) {
                                              if (data15 > 4294967295 || isNaN(data15)) {
                                                validate29.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                                return false;
                                              } else {
                                                if (data15 < 0 || isNaN(data15)) {
                                                  validate29.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
      validate29.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate29.errors = vErrors;
  return errors === 0;
}
function validate28(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.conversions === void 0 && (missing0 = "conversions")) {
        validate28.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "conversions" || key0 === "success")) {
            validate28.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate29(data0[i0], { instancePath: instancePath + "/conversions/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate29.errors : vErrors.concat(validate29.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate28.errors = [{ instancePath: instancePath + "/conversions", schemaPath: "#/properties/conversions/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate28.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate28.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate28.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate28.errors = vErrors;
  return errors === 0;
}
var validateConversionProgressResponse = validate31;
var schema36 = { "additionalProperties": false, "properties": { "bytesWritten": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "conversionId": { "type": "string" }, "currentTensor": { "type": ["string", "null"] }, "direction": { "$ref": "#/definitions/ConversionDirection" }, "error": { "enum": [null, "The model conversion did not complete successfully."], "type": ["string", "null"] }, "estimatedOutputSize": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "outputModelId": { "type": ["string", "null"] }, "pipelineStep": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "pipelineStepLabel": { "type": ["string", "null"] }, "pipelineStepsTotal": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "sourceModelId": { "type": "string" }, "status": { "$ref": "#/definitions/ConversionStatus" }, "targetQuant": { "type": ["string", "null"] }, "tensorsCompleted": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "tensorsTotal": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] } }, "required": ["conversionId", "sourceModelId", "direction", "status", "progress", "currentTensor", "tensorsCompleted", "tensorsTotal", "bytesWritten", "estimatedOutputSize", "targetQuant", "error", "outputModelId", "pipelineStep", "pipelineStepsTotal", "pipelineStepLabel"], "type": "object" };
function validate32(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.conversionId === void 0 && (missing0 = "conversionId") || data.sourceModelId === void 0 && (missing0 = "sourceModelId") || data.direction === void 0 && (missing0 = "direction") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.currentTensor === void 0 && (missing0 = "currentTensor") || data.tensorsCompleted === void 0 && (missing0 = "tensorsCompleted") || data.tensorsTotal === void 0 && (missing0 = "tensorsTotal") || data.bytesWritten === void 0 && (missing0 = "bytesWritten") || data.estimatedOutputSize === void 0 && (missing0 = "estimatedOutputSize") || data.targetQuant === void 0 && (missing0 = "targetQuant") || data.error === void 0 && (missing0 = "error") || data.outputModelId === void 0 && (missing0 = "outputModelId") || data.pipelineStep === void 0 && (missing0 = "pipelineStep") || data.pipelineStepsTotal === void 0 && (missing0 = "pipelineStepsTotal") || data.pipelineStepLabel === void 0 && (missing0 = "pipelineStepLabel")) {
        validate32.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema36.properties, key0)) {
            validate32.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.bytesWritten !== void 0) {
            let data0 = data.bytesWritten;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0)) && data0 !== null) {
              validate32.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/type", keyword: "type", params: { type: schema36.properties.bytesWritten.type }, message: "must be integer,null" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 9007199254740991 || isNaN(data0)) {
                  validate32.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                  return false;
                } else {
                  if (data0 < 0 || isNaN(data0)) {
                    validate32.errors = [{ instancePath: instancePath + "/bytesWritten", schemaPath: "#/properties/bytesWritten/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                validate32.errors = [{ instancePath: instancePath + "/conversionId", schemaPath: "#/properties/conversionId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                  validate32.errors = [{ instancePath: instancePath + "/currentTensor", schemaPath: "#/properties/currentTensor/type", keyword: "type", params: { type: schema36.properties.currentTensor.type }, message: "must be string,null" }];
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
                    validate32.errors = vErrors;
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
                      validate32.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema36.properties.error.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (!(data4 === null || data4 === "The model conversion did not complete successfully.")) {
                      validate32.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/enum", keyword: "enum", params: { allowedValues: schema36.properties.error.enum }, message: "must be equal to one of the allowed values" }];
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
                        validate32.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/type", keyword: "type", params: { type: schema36.properties.estimatedOutputSize.type }, message: "must be integer,null" }];
                        return false;
                      }
                      if (errors === _errs25) {
                        if (typeof data5 == "number" && isFinite(data5)) {
                          if (data5 > 9007199254740991 || isNaN(data5)) {
                            validate32.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                            return false;
                          } else {
                            if (data5 < 0 || isNaN(data5)) {
                              validate32.errors = [{ instancePath: instancePath + "/estimatedOutputSize", schemaPath: "#/properties/estimatedOutputSize/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                          validate32.errors = [{ instancePath: instancePath + "/outputModelId", schemaPath: "#/properties/outputModelId/type", keyword: "type", params: { type: schema36.properties.outputModelId.type }, message: "must be string,null" }];
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
                            validate32.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/type", keyword: "type", params: { type: schema36.properties.pipelineStep.type }, message: "must be integer,null" }];
                            return false;
                          }
                          if (errors === _errs29) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 4294967295 || isNaN(data7)) {
                                validate32.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate32.errors = [{ instancePath: instancePath + "/pipelineStep", schemaPath: "#/properties/pipelineStep/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                              validate32.errors = [{ instancePath: instancePath + "/pipelineStepLabel", schemaPath: "#/properties/pipelineStepLabel/type", keyword: "type", params: { type: schema36.properties.pipelineStepLabel.type }, message: "must be string,null" }];
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
                                validate32.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/type", keyword: "type", params: { type: schema36.properties.pipelineStepsTotal.type }, message: "must be integer,null" }];
                                return false;
                              }
                              if (errors === _errs33) {
                                if (typeof data9 == "number" && isFinite(data9)) {
                                  if (data9 > 4294967295 || isNaN(data9)) {
                                    validate32.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                    return false;
                                  } else {
                                    if (data9 < 0 || isNaN(data9)) {
                                      validate32.errors = [{ instancePath: instancePath + "/pipelineStepsTotal", schemaPath: "#/properties/pipelineStepsTotal/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                  validate32.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema36.properties.progress.type }, message: "must be number,null" }];
                                  return false;
                                }
                                if (errors === _errs35) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 1 || isNaN(data10)) {
                                      validate32.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate32.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate32.errors = [{ instancePath: instancePath + "/sourceModelId", schemaPath: "#/properties/sourceModelId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                      validate32.errors = vErrors;
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
                                        validate32.errors = [{ instancePath: instancePath + "/targetQuant", schemaPath: "#/properties/targetQuant/type", keyword: "type", params: { type: schema36.properties.targetQuant.type }, message: "must be string,null" }];
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
                                          validate32.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/type", keyword: "type", params: { type: schema36.properties.tensorsCompleted.type }, message: "must be integer,null" }];
                                          return false;
                                        }
                                        if (errors === _errs72) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 4294967295 || isNaN(data14)) {
                                              validate32.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate32.errors = [{ instancePath: instancePath + "/tensorsCompleted", schemaPath: "#/properties/tensorsCompleted/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                            validate32.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/type", keyword: "type", params: { type: schema36.properties.tensorsTotal.type }, message: "must be integer,null" }];
                                            return false;
                                          }
                                          if (errors === _errs74) {
                                            if (typeof data15 == "number" && isFinite(data15)) {
                                              if (data15 > 4294967295 || isNaN(data15)) {
                                                validate32.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                                return false;
                                              } else {
                                                if (data15 < 0 || isNaN(data15)) {
                                                  validate32.errors = [{ instancePath: instancePath + "/tensorsTotal", schemaPath: "#/properties/tensorsTotal/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
      validate32.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate32.errors = vErrors;
  return errors === 0;
}
function validate31(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.progress === void 0 && (missing0 = "progress")) {
        validate31.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "progress" || key0 === "success")) {
            validate31.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
            if (!validate32(data0, { instancePath: instancePath + "/progress", parentData: data, parentDataProperty: "progress", rootData })) {
              vErrors = vErrors === null ? validate32.errors : vErrors.concat(validate32.errors);
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
              validate31.errors = vErrors;
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
                validate31.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate31.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate31.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate31.errors = vErrors;
  return errors === 0;
}
var validateConversionSetupStartedOutcome = validate34;
var schema40 = { "additionalProperties": false, "properties": { "error": { "enum": [null, "Conversion environment setup did not complete successfully."], "type": ["string", "null"] }, "operationId": { "maxLength": 36, "minLength": 36, "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "type": "string" }, "status": { "$ref": "#/definitions/ConversionSetupStatus" } }, "pumasConversionSetup": true, "required": ["operationId", "status", "error"], "type": "object" };
var pattern12 = new RegExp("^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "u");
function validate35(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.operationId === void 0 && (missing0 = "operationId") || data.status === void 0 && (missing0 = "status") || data.error === void 0 && (missing0 = "error")) {
        validate35.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "error" || key0 === "operationId" || key0 === "status")) {
            validate35.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.error !== void 0) {
            let data0 = data.error;
            const _errs2 = errors;
            if (typeof data0 !== "string" && data0 !== null) {
              validate35.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema40.properties.error.type }, message: "must be string,null" }];
              return false;
            }
            if (!(data0 === null || data0 === "Conversion environment setup did not complete successfully.")) {
              validate35.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/enum", keyword: "enum", params: { allowedValues: schema40.properties.error.enum }, message: "must be equal to one of the allowed values" }];
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
                  if (func5(data1) > 36) {
                    validate35.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/maxLength", keyword: "maxLength", params: { limit: 36 }, message: "must NOT have more than 36 characters" }];
                    return false;
                  } else {
                    if (func5(data1) < 36) {
                      validate35.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/minLength", keyword: "minLength", params: { limit: 36 }, message: "must NOT have fewer than 36 characters" }];
                      return false;
                    } else {
                      if (!pattern12.test(data1)) {
                        validate35.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/pattern", keyword: "pattern", params: { pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$" }, message: 'must match pattern "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"' }];
                        return false;
                      }
                    }
                  }
                } else {
                  validate35.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                  validate35.errors = vErrors;
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
                  validate35.errors = [{ instancePath, schemaPath: "#/pumasConversionSetup", keyword: "pumasConversionSetup", params: {}, message: 'must pass "pumasConversionSetup" keyword validation' }];
                  return false;
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
      if (data.success === void 0 && (missing0 = "success") || data.setup === void 0 && (missing0 = "setup")) {
        validate34.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "setup" || key0 === "success")) {
            validate34.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.setup !== void 0) {
            const _errs2 = errors;
            if (!validate35(data.setup, { instancePath: instancePath + "/setup", parentData: data, parentDataProperty: "setup", rootData })) {
              vErrors = vErrors === null ? validate35.errors : vErrors.concat(validate35.errors);
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
                validate34.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate34.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate34.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate34.errors = vErrors;
  return errors === 0;
}
var validateConversionSetupStatusOutcome = validate37;
var schema43 = { "additionalProperties": false, "properties": { "error": { "enum": [null, "Conversion environment setup did not complete successfully."], "type": ["string", "null"] }, "operationId": { "maxLength": 36, "minLength": 36, "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "type": "string" }, "status": { "$ref": "#/definitions/ConversionSetupStatus" } }, "pumasConversionSetup": true, "required": ["operationId", "status", "error"], "type": "object" };
function validate38(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.operationId === void 0 && (missing0 = "operationId") || data.status === void 0 && (missing0 = "status") || data.error === void 0 && (missing0 = "error")) {
        validate38.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "error" || key0 === "operationId" || key0 === "status")) {
            validate38.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.error !== void 0) {
            let data0 = data.error;
            const _errs2 = errors;
            if (typeof data0 !== "string" && data0 !== null) {
              validate38.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema43.properties.error.type }, message: "must be string,null" }];
              return false;
            }
            if (!(data0 === null || data0 === "Conversion environment setup did not complete successfully.")) {
              validate38.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/enum", keyword: "enum", params: { allowedValues: schema43.properties.error.enum }, message: "must be equal to one of the allowed values" }];
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
                  if (func5(data1) > 36) {
                    validate38.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/maxLength", keyword: "maxLength", params: { limit: 36 }, message: "must NOT have more than 36 characters" }];
                    return false;
                  } else {
                    if (func5(data1) < 36) {
                      validate38.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/minLength", keyword: "minLength", params: { limit: 36 }, message: "must NOT have fewer than 36 characters" }];
                      return false;
                    } else {
                      if (!pattern12.test(data1)) {
                        validate38.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/pattern", keyword: "pattern", params: { pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$" }, message: 'must match pattern "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"' }];
                        return false;
                      }
                    }
                  }
                } else {
                  validate38.errors = [{ instancePath: instancePath + "/operationId", schemaPath: "#/properties/operationId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                  validate38.errors = vErrors;
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
                  validate38.errors = [{ instancePath, schemaPath: "#/pumasConversionSetup", keyword: "pumasConversionSetup", params: {}, message: 'must pass "pumasConversionSetup" keyword validation' }];
                  return false;
                }
              }
            }
          }
        }
      }
    } else {
      validate38.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate38.errors = vErrors;
  return errors === 0;
}
function validate37(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.setup === void 0 && (missing0 = "setup")) {
        validate37.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "setup" || key0 === "success")) {
            validate37.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
            if (!validate38(data0, { instancePath: instancePath + "/setup", parentData: data, parentDataProperty: "setup", rootData })) {
              vErrors = vErrors === null ? validate38.errors : vErrors.concat(validate38.errors);
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
              validate37.errors = vErrors;
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
                validate37.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate37.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate37.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate37.errors = vErrors;
  return errors === 0;
}
var validateConversionStartedOutcome = validate40;
var pattern14 = new RegExp("[^\\t\\n\\v\\f\\r \\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000]", "u");
function validate40(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.conversion_id === void 0 && (missing0 = "conversion_id")) {
        validate40.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "conversion_id" || key0 === "success")) {
            validate40.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  validate40.errors = [{ instancePath: instancePath + "/conversion_id", schemaPath: "#/properties/conversion_id/pattern", keyword: "pattern", params: { pattern: "[^\\t\\n\\v\\f\\r \\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000]" }, message: 'must match pattern "[^\\t\\n\\v\\f\\r \\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000]"' }];
                  return false;
                } else {
                  if (encodeURIComponent(data0).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                    validate40.errors = [{ instancePath: instancePath + "/conversion_id", schemaPath: "#/properties/conversion_id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate40.errors = [{ instancePath: instancePath + "/conversion_id", schemaPath: "#/properties/conversion_id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                validate40.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate40.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate40.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate40.errors = vErrors;
  return errors === 0;
}
var validateDownloadIdParams = validate41;
function validate41(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.download_id === void 0 && (missing0 = "download_id")) {
        validate41.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "download_id")) {
            validate41.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                if (func5(data0) < 1) {
                  validate41.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                  return false;
                } else {
                  if (encodeURIComponent(data0).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                    validate41.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate41.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
            }
          }
        }
      }
    } else {
      validate41.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate41.errors = vErrors;
  return errors === 0;
}
var validateDownloadListOutcome = validate42;
var schema48 = { "additionalProperties": false, "properties": { "downloadId": { "type": "string" }, "downloadedBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "error": { "type": ["string", "null"] }, "etaSeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "libraryModelId": { "pumasPortablePath": true, "pumasUtf8Max": 4096, "type": ["string", "null"] }, "modelName": { "type": ["string", "null"] }, "modelType": { "type": ["string", "null"] }, "nextRetryDelaySeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "repoId": { "type": ["string", "null"] }, "retryAttempt": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retryLimit": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retrying": { "type": ["boolean", "null"] }, "selectedArtifactId": { "type": ["string", "null"] }, "speed": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "status": { "$ref": "#/definitions/DownloadStatus" }, "totalBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["downloadId", "libraryModelId", "repoId", "selectedArtifactId", "modelName", "modelType", "status", "progress", "downloadedBytes", "totalBytes", "speed", "etaSeconds", "retryAttempt", "retryLimit", "retrying", "nextRetryDelaySeconds", "error"], "type": "object" };
var schema49 = { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" };
function validate43(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.downloadId === void 0 && (missing0 = "downloadId") || data.libraryModelId === void 0 && (missing0 = "libraryModelId") || data.repoId === void 0 && (missing0 = "repoId") || data.selectedArtifactId === void 0 && (missing0 = "selectedArtifactId") || data.modelName === void 0 && (missing0 = "modelName") || data.modelType === void 0 && (missing0 = "modelType") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.downloadedBytes === void 0 && (missing0 = "downloadedBytes") || data.totalBytes === void 0 && (missing0 = "totalBytes") || data.speed === void 0 && (missing0 = "speed") || data.etaSeconds === void 0 && (missing0 = "etaSeconds") || data.retryAttempt === void 0 && (missing0 = "retryAttempt") || data.retryLimit === void 0 && (missing0 = "retryLimit") || data.retrying === void 0 && (missing0 = "retrying") || data.nextRetryDelaySeconds === void 0 && (missing0 = "nextRetryDelaySeconds") || data.error === void 0 && (missing0 = "error")) {
        validate43.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema48.properties, key0)) {
            validate43.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.downloadId !== void 0) {
            const _errs2 = errors;
            if (typeof data.downloadId !== "string") {
              validate43.errors = [{ instancePath: instancePath + "/downloadId", schemaPath: "#/properties/downloadId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                validate43.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/type", keyword: "type", params: { type: schema48.properties.downloadedBytes.type }, message: "must be integer,null" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 9007199254740991 || isNaN(data1)) {
                    validate43.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate43.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                  validate43.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema48.properties.error.type }, message: "must be string,null" }];
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
                    validate43.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/type", keyword: "type", params: { type: schema48.properties.etaSeconds.type }, message: "must be number,null" }];
                    return false;
                  }
                  if (errors === _errs8) {
                    if (typeof data3 == "number" && isFinite(data3)) {
                      if (data3 > 17976931348623157e292 || isNaN(data3)) {
                        validate43.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                        return false;
                      } else {
                        if (data3 < 0 || isNaN(data3)) {
                          validate43.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate43.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/type", keyword: "type", params: { type: schema48.properties.libraryModelId.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (errors === _errs10) {
                      if (typeof data4 === "string") {
                        if (data4.length === 0 || data4.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(data4) || Array.from(data4).some((letter) => letter.codePointAt(0) < 32 || letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159) || data4.split("/").some((component) => {
                          const stem = component.split(".")[0].replace(/[a-z]/g, (letter) => letter.toUpperCase());
                          return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g, "x").length > 255 || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
                        })) {
                          validate43.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate43.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
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
                        validate43.errors = [{ instancePath: instancePath + "/modelName", schemaPath: "#/properties/modelName/type", keyword: "type", params: { type: schema48.properties.modelName.type }, message: "must be string,null" }];
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
                          validate43.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: schema48.properties.modelType.type }, message: "must be string,null" }];
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
                            validate43.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/type", keyword: "type", params: { type: schema48.properties.nextRetryDelaySeconds.type }, message: "must be number,null" }];
                            return false;
                          }
                          if (errors === _errs16) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 17976931348623157e292 || isNaN(data7)) {
                                validate43.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate43.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                              validate43.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema48.properties.progress.type }, message: "must be number,null" }];
                              return false;
                            }
                            if (errors === _errs18) {
                              if (typeof data8 == "number" && isFinite(data8)) {
                                if (data8 > 1 || isNaN(data8)) {
                                  validate43.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                  return false;
                                } else {
                                  if (data8 < 0 || isNaN(data8)) {
                                    validate43.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                validate43.errors = [{ instancePath: instancePath + "/repoId", schemaPath: "#/properties/repoId/type", keyword: "type", params: { type: schema48.properties.repoId.type }, message: "must be string,null" }];
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
                                  validate43.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/type", keyword: "type", params: { type: schema48.properties.retryAttempt.type }, message: "must be integer,null" }];
                                  return false;
                                }
                                if (errors === _errs22) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 4294967295 || isNaN(data10)) {
                                      validate43.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate43.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate43.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/type", keyword: "type", params: { type: schema48.properties.retryLimit.type }, message: "must be integer,null" }];
                                    return false;
                                  }
                                  if (errors === _errs24) {
                                    if (typeof data11 == "number" && isFinite(data11)) {
                                      if (data11 > 4294967295 || isNaN(data11)) {
                                        validate43.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                        return false;
                                      } else {
                                        if (data11 < 0 || isNaN(data11)) {
                                          validate43.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                      validate43.errors = [{ instancePath: instancePath + "/retrying", schemaPath: "#/properties/retrying/type", keyword: "type", params: { type: schema48.properties.retrying.type }, message: "must be boolean,null" }];
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
                                        validate43.errors = [{ instancePath: instancePath + "/selectedArtifactId", schemaPath: "#/properties/selectedArtifactId/type", keyword: "type", params: { type: schema48.properties.selectedArtifactId.type }, message: "must be string,null" }];
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
                                          validate43.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/type", keyword: "type", params: { type: schema48.properties.speed.type }, message: "must be number,null" }];
                                          return false;
                                        }
                                        if (errors === _errs30) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 17976931348623157e292 || isNaN(data14)) {
                                              validate43.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate43.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                            validate43.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                            return false;
                                          }
                                          if (!(data15 === "queued" || data15 === "downloading" || data15 === "pausing" || data15 === "paused" || data15 === "cancelling" || data15 === "completed" || data15 === "cancelled" || data15 === "error")) {
                                            validate43.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/enum", keyword: "enum", params: { allowedValues: schema49.enum }, message: "must be equal to one of the allowed values" }];
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
                                              validate43.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/type", keyword: "type", params: { type: schema48.properties.totalBytes.type }, message: "must be integer,null" }];
                                              return false;
                                            }
                                            if (errors === _errs35) {
                                              if (typeof data16 == "number" && isFinite(data16)) {
                                                if (data16 > 9007199254740991 || isNaN(data16)) {
                                                  validate43.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                                  return false;
                                                } else {
                                                  if (data16 < 0 || isNaN(data16)) {
                                                    validate43.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
      validate43.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate43.errors = vErrors;
  return errors === 0;
}
function validate42(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.downloads === void 0 && (missing0 = "downloads")) {
        validate42.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "downloads" || key0 === "success")) {
            validate42.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate43(data0[i0], { instancePath: instancePath + "/downloads/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate43.errors : vErrors.concat(validate43.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate42.errors = [{ instancePath: instancePath + "/downloads", schemaPath: "#/properties/downloads/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate42.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate42.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate42.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate42.errors = vErrors;
  return errors === 0;
}
var validateDownloadMutationOutcome = validate45;
function validate45(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success")) {
        validate45.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "error" || key0 === "success")) {
            validate45.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.error !== void 0) {
            const _errs2 = errors;
            if (typeof data.error !== "string") {
              validate45.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                validate45.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.success === true && data.error !== void 0 || data.success === false && typeof data.error !== "string") {
                validate45.errors = [{ instancePath, schemaPath: "#/pumasMutation", keyword: "pumasMutation", params: {}, message: 'must pass "pumasMutation" keyword validation' }];
                return false;
              }
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
var validateDownloadStartedOutcome = validate46;
var schema52 = { "additionalProperties": false, "properties": { "artifactId": { "type": ["string", "null"] }, "download_id": { "type": "string" }, "selectedArtifactId": { "type": ["string", "null"] }, "success": { "const": true, "type": "boolean" } }, "pumasStarted": true, "required": ["success", "download_id", "selectedArtifactId", "artifactId"], "type": "object" };
function validate46(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
              const err2 = { instancePath: instancePath + "/artifactId", schemaPath: "#/definitions/DownloadStartedSuccess/properties/artifactId/type", keyword: "type", params: { type: schema52.properties.artifactId.type }, message: "must be string,null" };
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
                  const err4 = { instancePath: instancePath + "/selectedArtifactId", schemaPath: "#/definitions/DownloadStartedSuccess/properties/selectedArtifactId/type", keyword: "type", params: { type: schema52.properties.selectedArtifactId.type }, message: "must be string,null" };
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
    validate46.errors = vErrors;
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
  validate46.errors = vErrors;
  return errors === 0;
}
var validateDownloadStatusOutcome = validate47;
var schema55 = { "additionalProperties": false, "properties": { "downloadId": { "type": "string" }, "downloadedBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "error": { "type": ["string", "null"] }, "etaSeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "libraryModelId": { "pumasPortablePath": true, "pumasUtf8Max": 4096, "type": ["string", "null"] }, "modelName": { "type": ["string", "null"] }, "modelType": { "type": ["string", "null"] }, "nextRetryDelaySeconds": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "progress": { "maximum": 1, "minimum": 0, "type": ["number", "null"] }, "repoId": { "type": ["string", "null"] }, "retryAttempt": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retryLimit": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "retrying": { "type": ["boolean", "null"] }, "selectedArtifactId": { "type": ["string", "null"] }, "speed": { "maximum": 17976931348623157e292, "minimum": 0, "type": ["number", "null"] }, "status": { "$ref": "#/definitions/DownloadStatus" }, "success": { "const": true, "type": "boolean" }, "totalBytes": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["success", "downloadId", "libraryModelId", "repoId", "selectedArtifactId", "modelName", "modelType", "status", "progress", "downloadedBytes", "totalBytes", "speed", "etaSeconds", "retryAttempt", "retryLimit", "retrying", "nextRetryDelaySeconds", "error"], "type": "object" };
var schema56 = { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" };
function validate48(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.downloadId === void 0 && (missing0 = "downloadId") || data.libraryModelId === void 0 && (missing0 = "libraryModelId") || data.repoId === void 0 && (missing0 = "repoId") || data.selectedArtifactId === void 0 && (missing0 = "selectedArtifactId") || data.modelName === void 0 && (missing0 = "modelName") || data.modelType === void 0 && (missing0 = "modelType") || data.status === void 0 && (missing0 = "status") || data.progress === void 0 && (missing0 = "progress") || data.downloadedBytes === void 0 && (missing0 = "downloadedBytes") || data.totalBytes === void 0 && (missing0 = "totalBytes") || data.speed === void 0 && (missing0 = "speed") || data.etaSeconds === void 0 && (missing0 = "etaSeconds") || data.retryAttempt === void 0 && (missing0 = "retryAttempt") || data.retryLimit === void 0 && (missing0 = "retryLimit") || data.retrying === void 0 && (missing0 = "retrying") || data.nextRetryDelaySeconds === void 0 && (missing0 = "nextRetryDelaySeconds") || data.error === void 0 && (missing0 = "error")) {
        validate48.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema55.properties, key0)) {
            validate48.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.downloadId !== void 0) {
            const _errs2 = errors;
            if (typeof data.downloadId !== "string") {
              validate48.errors = [{ instancePath: instancePath + "/downloadId", schemaPath: "#/properties/downloadId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                validate48.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/type", keyword: "type", params: { type: schema55.properties.downloadedBytes.type }, message: "must be integer,null" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 9007199254740991 || isNaN(data1)) {
                    validate48.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate48.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                  validate48.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema55.properties.error.type }, message: "must be string,null" }];
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
                    validate48.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/type", keyword: "type", params: { type: schema55.properties.etaSeconds.type }, message: "must be number,null" }];
                    return false;
                  }
                  if (errors === _errs8) {
                    if (typeof data3 == "number" && isFinite(data3)) {
                      if (data3 > 17976931348623157e292 || isNaN(data3)) {
                        validate48.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                        return false;
                      } else {
                        if (data3 < 0 || isNaN(data3)) {
                          validate48.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate48.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/type", keyword: "type", params: { type: schema55.properties.libraryModelId.type }, message: "must be string,null" }];
                      return false;
                    }
                    if (errors === _errs10) {
                      if (typeof data4 === "string") {
                        if (data4.length === 0 || data4.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(data4) || Array.from(data4).some((letter) => letter.codePointAt(0) < 32 || letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159) || data4.split("/").some((component) => {
                          const stem = component.split(".")[0].replace(/[a-z]/g, (letter) => letter.toUpperCase());
                          return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g, "x").length > 255 || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem);
                        })) {
                          validate48.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate48.errors = [{ instancePath: instancePath + "/libraryModelId", schemaPath: "#/properties/libraryModelId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
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
                        validate48.errors = [{ instancePath: instancePath + "/modelName", schemaPath: "#/properties/modelName/type", keyword: "type", params: { type: schema55.properties.modelName.type }, message: "must be string,null" }];
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
                          validate48.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: schema55.properties.modelType.type }, message: "must be string,null" }];
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
                            validate48.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/type", keyword: "type", params: { type: schema55.properties.nextRetryDelaySeconds.type }, message: "must be number,null" }];
                            return false;
                          }
                          if (errors === _errs16) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 17976931348623157e292 || isNaN(data7)) {
                                validate48.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate48.errors = [{ instancePath: instancePath + "/nextRetryDelaySeconds", schemaPath: "#/properties/nextRetryDelaySeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                              validate48.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/type", keyword: "type", params: { type: schema55.properties.progress.type }, message: "must be number,null" }];
                              return false;
                            }
                            if (errors === _errs18) {
                              if (typeof data8 == "number" && isFinite(data8)) {
                                if (data8 > 1 || isNaN(data8)) {
                                  validate48.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1 }, message: "must be <= 1" }];
                                  return false;
                                } else {
                                  if (data8 < 0 || isNaN(data8)) {
                                    validate48.errors = [{ instancePath: instancePath + "/progress", schemaPath: "#/properties/progress/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                validate48.errors = [{ instancePath: instancePath + "/repoId", schemaPath: "#/properties/repoId/type", keyword: "type", params: { type: schema55.properties.repoId.type }, message: "must be string,null" }];
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
                                  validate48.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/type", keyword: "type", params: { type: schema55.properties.retryAttempt.type }, message: "must be integer,null" }];
                                  return false;
                                }
                                if (errors === _errs22) {
                                  if (typeof data10 == "number" && isFinite(data10)) {
                                    if (data10 > 4294967295 || isNaN(data10)) {
                                      validate48.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate48.errors = [{ instancePath: instancePath + "/retryAttempt", schemaPath: "#/properties/retryAttempt/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate48.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/type", keyword: "type", params: { type: schema55.properties.retryLimit.type }, message: "must be integer,null" }];
                                    return false;
                                  }
                                  if (errors === _errs24) {
                                    if (typeof data11 == "number" && isFinite(data11)) {
                                      if (data11 > 4294967295 || isNaN(data11)) {
                                        validate48.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                        return false;
                                      } else {
                                        if (data11 < 0 || isNaN(data11)) {
                                          validate48.errors = [{ instancePath: instancePath + "/retryLimit", schemaPath: "#/properties/retryLimit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                      validate48.errors = [{ instancePath: instancePath + "/retrying", schemaPath: "#/properties/retrying/type", keyword: "type", params: { type: schema55.properties.retrying.type }, message: "must be boolean,null" }];
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
                                        validate48.errors = [{ instancePath: instancePath + "/selectedArtifactId", schemaPath: "#/properties/selectedArtifactId/type", keyword: "type", params: { type: schema55.properties.selectedArtifactId.type }, message: "must be string,null" }];
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
                                          validate48.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/type", keyword: "type", params: { type: schema55.properties.speed.type }, message: "must be number,null" }];
                                          return false;
                                        }
                                        if (errors === _errs30) {
                                          if (typeof data14 == "number" && isFinite(data14)) {
                                            if (data14 > 17976931348623157e292 || isNaN(data14)) {
                                              validate48.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                              return false;
                                            } else {
                                              if (data14 < 0 || isNaN(data14)) {
                                                validate48.errors = [{ instancePath: instancePath + "/speed", schemaPath: "#/properties/speed/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                            validate48.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                            return false;
                                          }
                                          if (!(data15 === "queued" || data15 === "downloading" || data15 === "pausing" || data15 === "paused" || data15 === "cancelling" || data15 === "completed" || data15 === "cancelled" || data15 === "error")) {
                                            validate48.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/enum", keyword: "enum", params: { allowedValues: schema56.enum }, message: "must be equal to one of the allowed values" }];
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
                                              validate48.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                              return false;
                                            }
                                            if (true !== data16) {
                                              validate48.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
                                                validate48.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/type", keyword: "type", params: { type: schema55.properties.totalBytes.type }, message: "must be integer,null" }];
                                                return false;
                                              }
                                              if (errors === _errs37) {
                                                if (typeof data17 == "number" && isFinite(data17)) {
                                                  if (data17 > 9007199254740991 || isNaN(data17)) {
                                                    validate48.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                                    return false;
                                                  } else {
                                                    if (data17 < 0 || isNaN(data17)) {
                                                      validate48.errors = [{ instancePath: instancePath + "/totalBytes", schemaPath: "#/properties/totalBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
      validate48.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate48.errors = vErrors;
  return errors === 0;
}
function validate47(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (!validate48(data, { instancePath, parentData, parentDataProperty, rootData })) {
    vErrors = vErrors === null ? validate48.errors : vErrors.concat(validate48.errors);
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
    validate47.errors = vErrors;
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
  validate47.errors = vErrors;
  return errors === 0;
}
var validateGetBackendSetupParams = validate50;
function validate50(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.backend === void 0 && (missing0 = "backend")) {
        validate50.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backend")) {
            validate50.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
              validate50.errors = vErrors;
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
      validate50.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate50.errors = vErrors;
  return errors === 0;
}
var validateGetHfDownloadDetailsParams = validate51;
var schema60 = { "anyOf": [{ "additionalProperties": false, "properties": { "quants": { "default": null, "items": { "type": "string" }, "type": ["array", "null"] }, "repo_id": { "type": "string" } }, "required": ["repo_id"], "type": "object" }, { "additionalProperties": false, "properties": { "quants": { "default": null, "items": { "type": "string" }, "type": ["array", "null"] }, "repoId": { "type": "string" } }, "required": ["repoId"], "type": "object" }] };
function validate51(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
              const err2 = { instancePath: instancePath + "/quants", schemaPath: "#/anyOf/0/properties/quants/type", keyword: "type", params: { type: schema60.anyOf[0].properties.quants.type }, message: "must be array,null" };
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
                const err8 = { instancePath: instancePath + "/quants", schemaPath: "#/anyOf/1/properties/quants/type", keyword: "type", params: { type: schema60.anyOf[1].properties.quants.type }, message: "must be array,null" };
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
    validate51.errors = vErrors;
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
  validate51.errors = vErrors;
  return errors === 0;
}
var validateGetReleaseDependenciesOutcome = validate52;
function validate52(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.dependencies === void 0 && (missing0 = "dependencies")) {
        validate52.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "dependencies" || key0 === "success")) {
            validate52.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.dependencies !== void 0) {
            let data0 = data.dependencies;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (typeof data0[i0] !== "string") {
                    validate52.errors = [{ instancePath: instancePath + "/dependencies/" + i0, schemaPath: "#/properties/dependencies/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate52.errors = [{ instancePath: instancePath + "/dependencies", schemaPath: "#/properties/dependencies/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate52.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate52.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate52.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate52.errors = vErrors;
  return errors === 0;
}
var validateGetReleaseDependenciesParams = validate53;
function validate53(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.app_id === void 0 && (missing0 = "app_id") || data.tag === void 0 && (missing0 = "tag")) {
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
          if (!(key0 === "app_id" || key0 === "tag")) {
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
          if (data.app_id !== void 0) {
            const _errs4 = errors;
            if (typeof data.app_id !== "string") {
              const err2 = { instancePath: instancePath + "/app_id", schemaPath: "#/anyOf/0/properties/app_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.tag !== void 0) {
              const _errs6 = errors;
              if (typeof data.tag !== "string") {
                const err3 = { instancePath: instancePath + "/tag", schemaPath: "#/anyOf/0/properties/tag/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err3];
                } else {
                  vErrors.push(err3);
                }
                errors++;
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      const err4 = { instancePath, schemaPath: "#/anyOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs8 = errors;
    if (errors === _errs8) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.appId === void 0 && (missing1 = "appId") || data.tag === void 0 && (missing1 = "tag")) {
          const err5 = { instancePath, schemaPath: "#/anyOf/1/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        } else {
          const _errs10 = errors;
          for (const key1 in data) {
            if (!(key1 === "appId" || key1 === "tag")) {
              const err6 = { instancePath, schemaPath: "#/anyOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err6];
              } else {
                vErrors.push(err6);
              }
              errors++;
              break;
            }
          }
          if (_errs10 === errors) {
            if (data.appId !== void 0) {
              const _errs11 = errors;
              if (typeof data.appId !== "string") {
                const err7 = { instancePath: instancePath + "/appId", schemaPath: "#/anyOf/1/properties/appId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err7];
                } else {
                  vErrors.push(err7);
                }
                errors++;
              }
              var valid2 = _errs11 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.tag !== void 0) {
                const _errs13 = errors;
                if (typeof data.tag !== "string") {
                  const err8 = { instancePath: instancePath + "/tag", schemaPath: "#/anyOf/1/properties/tag/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err8];
                  } else {
                    vErrors.push(err8);
                  }
                  errors++;
                }
                var valid2 = _errs13 === errors;
              } else {
                var valid2 = true;
              }
            }
          }
        }
      } else {
        const err9 = { instancePath, schemaPath: "#/anyOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    var _valid0 = _errs8 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
    validate53.errors = vErrors;
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
  validate53.errors = vErrors;
  return errors === 0;
}
var validateGithubCacheStatusOutcome = validate54;
var schema64 = { "additionalProperties": false, "properties": { "age_seconds": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "has_cache": { "type": "boolean" }, "is_fetching": { "type": "boolean" }, "is_valid": { "type": "boolean" }, "last_fetched": { "type": ["string", "null"] }, "releases_count": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] } }, "required": ["has_cache", "is_valid", "is_fetching", "age_seconds", "last_fetched", "releases_count"], "type": "object" };
function validate54(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  const _errs2 = errors;
  if (errors === _errs2) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.has_cache === void 0 && (missing0 = "has_cache") || data.is_valid === void 0 && (missing0 = "is_valid") || data.is_fetching === void 0 && (missing0 = "is_fetching") || data.age_seconds === void 0 && (missing0 = "age_seconds") || data.last_fetched === void 0 && (missing0 = "last_fetched") || data.releases_count === void 0 && (missing0 = "releases_count")) {
        const err0 = { instancePath, schemaPath: "#/definitions/GithubCacheStatusSnapshot/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs4 = errors;
        for (const key0 in data) {
          if (!(key0 === "age_seconds" || key0 === "has_cache" || key0 === "is_fetching" || key0 === "is_valid" || key0 === "last_fetched" || key0 === "releases_count")) {
            const err1 = { instancePath, schemaPath: "#/definitions/GithubCacheStatusSnapshot/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
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
          if (data.age_seconds !== void 0) {
            let data0 = data.age_seconds;
            const _errs5 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0)) && data0 !== null) {
              const err2 = { instancePath: instancePath + "/age_seconds", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/age_seconds/type", keyword: "type", params: { type: schema64.properties.age_seconds.type }, message: "must be integer,null" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if (errors === _errs5) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 9007199254740991 || isNaN(data0)) {
                  const err3 = { instancePath: instancePath + "/age_seconds", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/age_seconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" };
                  if (vErrors === null) {
                    vErrors = [err3];
                  } else {
                    vErrors.push(err3);
                  }
                  errors++;
                } else {
                  if (data0 < 0 || isNaN(data0)) {
                    const err4 = { instancePath: instancePath + "/age_seconds", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/age_seconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                    if (vErrors === null) {
                      vErrors = [err4];
                    } else {
                      vErrors.push(err4);
                    }
                    errors++;
                  }
                }
              }
            }
            var valid2 = _errs5 === errors;
          } else {
            var valid2 = true;
          }
          if (valid2) {
            if (data.has_cache !== void 0) {
              const _errs7 = errors;
              if (typeof data.has_cache !== "boolean") {
                const err5 = { instancePath: instancePath + "/has_cache", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/has_cache/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                if (vErrors === null) {
                  vErrors = [err5];
                } else {
                  vErrors.push(err5);
                }
                errors++;
              }
              var valid2 = _errs7 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.is_fetching !== void 0) {
                const _errs9 = errors;
                if (typeof data.is_fetching !== "boolean") {
                  const err6 = { instancePath: instancePath + "/is_fetching", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/is_fetching/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                  if (vErrors === null) {
                    vErrors = [err6];
                  } else {
                    vErrors.push(err6);
                  }
                  errors++;
                }
                var valid2 = _errs9 === errors;
              } else {
                var valid2 = true;
              }
              if (valid2) {
                if (data.is_valid !== void 0) {
                  const _errs11 = errors;
                  if (typeof data.is_valid !== "boolean") {
                    const err7 = { instancePath: instancePath + "/is_valid", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/is_valid/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                    if (vErrors === null) {
                      vErrors = [err7];
                    } else {
                      vErrors.push(err7);
                    }
                    errors++;
                  }
                  var valid2 = _errs11 === errors;
                } else {
                  var valid2 = true;
                }
                if (valid2) {
                  if (data.last_fetched !== void 0) {
                    let data4 = data.last_fetched;
                    const _errs13 = errors;
                    if (typeof data4 !== "string" && data4 !== null) {
                      const err8 = { instancePath: instancePath + "/last_fetched", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/last_fetched/type", keyword: "type", params: { type: schema64.properties.last_fetched.type }, message: "must be string,null" };
                      if (vErrors === null) {
                        vErrors = [err8];
                      } else {
                        vErrors.push(err8);
                      }
                      errors++;
                    }
                    var valid2 = _errs13 === errors;
                  } else {
                    var valid2 = true;
                  }
                  if (valid2) {
                    if (data.releases_count !== void 0) {
                      let data5 = data.releases_count;
                      const _errs15 = errors;
                      if (!(typeof data5 == "number" && (!(data5 % 1) && !isNaN(data5)) && isFinite(data5)) && data5 !== null) {
                        const err9 = { instancePath: instancePath + "/releases_count", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/releases_count/type", keyword: "type", params: { type: schema64.properties.releases_count.type }, message: "must be integer,null" };
                        if (vErrors === null) {
                          vErrors = [err9];
                        } else {
                          vErrors.push(err9);
                        }
                        errors++;
                      }
                      if (errors === _errs15) {
                        if (typeof data5 == "number" && isFinite(data5)) {
                          if (data5 > 4294967295 || isNaN(data5)) {
                            const err10 = { instancePath: instancePath + "/releases_count", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/releases_count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" };
                            if (vErrors === null) {
                              vErrors = [err10];
                            } else {
                              vErrors.push(err10);
                            }
                            errors++;
                          } else {
                            if (data5 < 0 || isNaN(data5)) {
                              const err11 = { instancePath: instancePath + "/releases_count", schemaPath: "#/definitions/GithubCacheStatusSnapshot/properties/releases_count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                              if (vErrors === null) {
                                vErrors = [err11];
                              } else {
                                vErrors.push(err11);
                              }
                              errors++;
                            }
                          }
                        }
                      }
                      var valid2 = _errs15 === errors;
                    } else {
                      var valid2 = true;
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      const err12 = { instancePath, schemaPath: "#/definitions/GithubCacheStatusSnapshot/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err12];
      } else {
        vErrors.push(err12);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs17 = errors;
    const _errs18 = errors;
    if (errors === _errs18) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.has_cache === void 0 && (missing1 = "has_cache") || data.is_valid === void 0 && (missing1 = "is_valid") || data.is_fetching === void 0 && (missing1 = "is_fetching")) {
          const err13 = { instancePath, schemaPath: "#/definitions/GithubCacheStatusNoManager/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err13];
          } else {
            vErrors.push(err13);
          }
          errors++;
        } else {
          const _errs20 = errors;
          for (const key1 in data) {
            if (!(key1 === "has_cache" || key1 === "is_fetching" || key1 === "is_valid")) {
              const err14 = { instancePath, schemaPath: "#/definitions/GithubCacheStatusNoManager/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err14];
              } else {
                vErrors.push(err14);
              }
              errors++;
              break;
            }
          }
          if (_errs20 === errors) {
            if (data.has_cache !== void 0) {
              let data6 = data.has_cache;
              const _errs21 = errors;
              if (typeof data6 !== "boolean") {
                const err15 = { instancePath: instancePath + "/has_cache", schemaPath: "#/definitions/GithubCacheStatusNoManager/properties/has_cache/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                if (vErrors === null) {
                  vErrors = [err15];
                } else {
                  vErrors.push(err15);
                }
                errors++;
              }
              if (false !== data6) {
                const err16 = { instancePath: instancePath + "/has_cache", schemaPath: "#/definitions/GithubCacheStatusNoManager/properties/has_cache/const", keyword: "const", params: { allowedValue: false }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err16];
                } else {
                  vErrors.push(err16);
                }
                errors++;
              }
              var valid4 = _errs21 === errors;
            } else {
              var valid4 = true;
            }
            if (valid4) {
              if (data.is_fetching !== void 0) {
                let data7 = data.is_fetching;
                const _errs23 = errors;
                if (typeof data7 !== "boolean") {
                  const err17 = { instancePath: instancePath + "/is_fetching", schemaPath: "#/definitions/GithubCacheStatusNoManager/properties/is_fetching/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                  if (vErrors === null) {
                    vErrors = [err17];
                  } else {
                    vErrors.push(err17);
                  }
                  errors++;
                }
                if (false !== data7) {
                  const err18 = { instancePath: instancePath + "/is_fetching", schemaPath: "#/definitions/GithubCacheStatusNoManager/properties/is_fetching/const", keyword: "const", params: { allowedValue: false }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err18];
                  } else {
                    vErrors.push(err18);
                  }
                  errors++;
                }
                var valid4 = _errs23 === errors;
              } else {
                var valid4 = true;
              }
              if (valid4) {
                if (data.is_valid !== void 0) {
                  let data8 = data.is_valid;
                  const _errs25 = errors;
                  if (typeof data8 !== "boolean") {
                    const err19 = { instancePath: instancePath + "/is_valid", schemaPath: "#/definitions/GithubCacheStatusNoManager/properties/is_valid/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                    if (vErrors === null) {
                      vErrors = [err19];
                    } else {
                      vErrors.push(err19);
                    }
                    errors++;
                  }
                  if (false !== data8) {
                    const err20 = { instancePath: instancePath + "/is_valid", schemaPath: "#/definitions/GithubCacheStatusNoManager/properties/is_valid/const", keyword: "const", params: { allowedValue: false }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err20];
                    } else {
                      vErrors.push(err20);
                    }
                    errors++;
                  }
                  var valid4 = _errs25 === errors;
                } else {
                  var valid4 = true;
                }
              }
            }
          }
        }
      } else {
        const err21 = { instancePath, schemaPath: "#/definitions/GithubCacheStatusNoManager/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err21];
        } else {
          vErrors.push(err21);
        }
        errors++;
      }
    }
    var _valid0 = _errs17 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err22 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err22];
    } else {
      vErrors.push(err22);
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
var validateHfDownloadDetailsOutcome = validate55;
var schema68 = { "additionalProperties": false, "description": "Exact download details derived from a repository file tree.", "properties": { "downloadOptions": { "default": [], "items": { "$ref": "#/definitions/DownloadOption" }, "type": "array" }, "repoId": { "type": "string" }, "totalSizeBytes": { "default": null, "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["repoId", "downloadOptions", "totalSizeBytes"], "type": "object" };
var schema69 = { "additionalProperties": false, "description": "Download option for a quantization variant or file group.", "properties": { "fileGroup": { "$ref": "#/definitions/FileGroup" }, "quant": { "type": "string" }, "sizeBytes": { "default": null, "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["quant", "sizeBytes"], "type": "object" };
function validate58(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.quant === void 0 && (missing0 = "quant") || data.sizeBytes === void 0 && (missing0 = "sizeBytes")) {
        validate58.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "fileGroup" || key0 === "quant" || key0 === "sizeBytes")) {
            validate58.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  validate58.errors = [{ instancePath: instancePath + "/fileGroup", schemaPath: "#/definitions/FileGroup/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" }];
                  return false;
                } else {
                  const _errs5 = errors;
                  for (const key1 in data0) {
                    if (!(key1 === "filenames" || key1 === "label" || key1 === "shardCount")) {
                      validate58.errors = [{ instancePath: instancePath + "/fileGroup", schemaPath: "#/definitions/FileGroup/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" }];
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
                              validate58.errors = [{ instancePath: instancePath + "/fileGroup/filenames/" + i0, schemaPath: "#/definitions/FileGroup/properties/filenames/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                              return false;
                            }
                            var valid3 = _errs8 === errors;
                            if (!valid3) {
                              break;
                            }
                          }
                        } else {
                          validate58.errors = [{ instancePath: instancePath + "/fileGroup/filenames", schemaPath: "#/definitions/FileGroup/properties/filenames/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                          validate58.errors = [{ instancePath: instancePath + "/fileGroup/label", schemaPath: "#/definitions/FileGroup/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                            validate58.errors = [{ instancePath: instancePath + "/fileGroup/shardCount", schemaPath: "#/definitions/FileGroup/properties/shardCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                            return false;
                          }
                          if (errors === _errs12) {
                            if (typeof data4 == "number" && isFinite(data4)) {
                              if (data4 > 4294967295 || isNaN(data4)) {
                                validate58.errors = [{ instancePath: instancePath + "/fileGroup/shardCount", schemaPath: "#/definitions/FileGroup/properties/shardCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                                return false;
                              } else {
                                if (data4 < 0 || isNaN(data4)) {
                                  validate58.errors = [{ instancePath: instancePath + "/fileGroup/shardCount", schemaPath: "#/definitions/FileGroup/properties/shardCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                validate58.errors = [{ instancePath: instancePath + "/fileGroup", schemaPath: "#/definitions/FileGroup/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
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
                validate58.errors = [{ instancePath: instancePath + "/quant", schemaPath: "#/properties/quant/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                  validate58.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/type", keyword: "type", params: { type: schema69.properties.sizeBytes.type }, message: "must be integer,null" }];
                  return false;
                }
                if (errors === _errs16) {
                  if (typeof data6 == "number" && isFinite(data6)) {
                    if (data6 > 9007199254740991 || isNaN(data6)) {
                      validate58.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                      return false;
                    } else {
                      if (data6 < 0 || isNaN(data6)) {
                        validate58.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
      validate58.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate58.errors = vErrors;
  return errors === 0;
}
function validate57(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.repoId === void 0 && (missing0 = "repoId") || data.downloadOptions === void 0 && (missing0 = "downloadOptions") || data.totalSizeBytes === void 0 && (missing0 = "totalSizeBytes")) {
        validate57.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "downloadOptions" || key0 === "repoId" || key0 === "totalSizeBytes")) {
            validate57.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate58(data0[i0], { instancePath: instancePath + "/downloadOptions/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate58.errors : vErrors.concat(validate58.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate57.errors = [{ instancePath: instancePath + "/downloadOptions", schemaPath: "#/properties/downloadOptions/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate57.errors = [{ instancePath: instancePath + "/repoId", schemaPath: "#/properties/repoId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                  validate57.errors = [{ instancePath: instancePath + "/totalSizeBytes", schemaPath: "#/properties/totalSizeBytes/type", keyword: "type", params: { type: schema68.properties.totalSizeBytes.type }, message: "must be integer,null" }];
                  return false;
                }
                if (errors === _errs7) {
                  if (typeof data3 == "number" && isFinite(data3)) {
                    if (data3 > 9007199254740991 || isNaN(data3)) {
                      validate57.errors = [{ instancePath: instancePath + "/totalSizeBytes", schemaPath: "#/properties/totalSizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                      return false;
                    } else {
                      if (data3 < 0 || isNaN(data3)) {
                        validate57.errors = [{ instancePath: instancePath + "/totalSizeBytes", schemaPath: "#/properties/totalSizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
      validate57.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate57.errors = vErrors;
  return errors === 0;
}
function validate56(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.details === void 0 && (missing0 = "details")) {
        validate56.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "details" || key0 === "success")) {
            validate56.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.details !== void 0) {
            const _errs2 = errors;
            if (!validate57(data.details, { instancePath: instancePath + "/details", parentData: data, parentDataProperty: "details", rootData })) {
              vErrors = vErrors === null ? validate57.errors : vErrors.concat(validate57.errors);
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
                validate56.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate56.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate56.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate56.errors = vErrors;
  return errors === 0;
}
function validate55(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (!validate56(data, { instancePath, parentData, parentDataProperty, rootData })) {
    vErrors = vErrors === null ? validate56.errors : vErrors.concat(validate56.errors);
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
    validate55.errors = vErrors;
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
  validate55.errors = vErrors;
  return errors === 0;
}
var validateInferenceSettingsOutcome = validate62;
var schema73 = { "additionalProperties": false, "description": "Describes a single configurable inference parameter with its type,\ndefault value, and optional constraints.\n\nDownstream consumers (e.g. Pantograph node graph) use this schema\nto dynamically render UI controls for model-specific settings.", "properties": { "constraints": { "anyOf": [{ "$ref": "#/definitions/ParamConstraints" }, { "type": "null" }], "default": null, "description": "Optional numeric/enum constraints." }, "default": { "$ref": "#/definitions/DesktopJsonValue" }, "description": { "default": null, "description": "Optional description / tooltip.", "type": ["string", "null"] }, "key": { "description": 'Machine-readable key (e.g. "context_length", "denoising_steps").', "type": "string" }, "label": { "description": 'Human-readable label (e.g. "Context Length").', "type": "string" }, "param_type": { "allOf": [{ "$ref": "#/definitions/ParamType" }], "description": "Data type of this parameter." } }, "required": ["key", "label", "param_type", "default", "description", "constraints"], "type": "object" };
var schema76 = { "description": "Data type for an inference parameter.", "enum": ["Number", "Integer", "String", "Boolean"], "type": "string" };
var schema74 = { "additionalProperties": false, "description": "Constraints on an inference parameter value.", "properties": { "allowed_values": { "anyOf": [{ "type": "null" }, { "items": { "$ref": "#/definitions/DesktopJsonValue" }, "type": "array" }] }, "max": { "default": null, "maximum": 17976931348623157e292, "minimum": -17976931348623157e292, "type": ["number", "null"] }, "min": { "default": null, "maximum": 17976931348623157e292, "minimum": -17976931348623157e292, "type": ["number", "null"] } }, "required": ["min", "max", "allowed_values"], "type": "object" };
var wrapper0 = { validate: validate65 };
function validate65(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
    validate65.errors = vErrors;
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
  validate65.errors = vErrors;
  return errors === 0;
}
function validate64(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.min === void 0 && (missing0 = "min") || data.max === void 0 && (missing0 = "max") || data.allowed_values === void 0 && (missing0 = "allowed_values")) {
        validate64.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "allowed_values" || key0 === "max" || key0 === "min")) {
            validate64.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                    if (!validate65(data0[i0], { instancePath: instancePath + "/allowed_values/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                      vErrors = vErrors === null ? validate65.errors : vErrors.concat(validate65.errors);
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
              validate64.errors = vErrors;
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
                validate64.errors = [{ instancePath: instancePath + "/max", schemaPath: "#/properties/max/type", keyword: "type", params: { type: schema74.properties.max.type }, message: "must be number,null" }];
                return false;
              }
              if (errors === _errs9) {
                if (typeof data2 == "number" && isFinite(data2)) {
                  if (data2 > 17976931348623157e292 || isNaN(data2)) {
                    validate64.errors = [{ instancePath: instancePath + "/max", schemaPath: "#/properties/max/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                    return false;
                  } else {
                    if (data2 < -17976931348623157e292 || isNaN(data2)) {
                      validate64.errors = [{ instancePath: instancePath + "/max", schemaPath: "#/properties/max/minimum", keyword: "minimum", params: { comparison: ">=", limit: -17976931348623157e292 }, message: "must be >= -1.7976931348623157e+308" }];
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
                  validate64.errors = [{ instancePath: instancePath + "/min", schemaPath: "#/properties/min/type", keyword: "type", params: { type: schema74.properties.min.type }, message: "must be number,null" }];
                  return false;
                }
                if (errors === _errs11) {
                  if (typeof data3 == "number" && isFinite(data3)) {
                    if (data3 > 17976931348623157e292 || isNaN(data3)) {
                      validate64.errors = [{ instancePath: instancePath + "/min", schemaPath: "#/properties/min/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                      return false;
                    } else {
                      if (data3 < -17976931348623157e292 || isNaN(data3)) {
                        validate64.errors = [{ instancePath: instancePath + "/min", schemaPath: "#/properties/min/minimum", keyword: "minimum", params: { comparison: ">=", limit: -17976931348623157e292 }, message: "must be >= -1.7976931348623157e+308" }];
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
      validate64.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate64.errors = vErrors;
  return errors === 0;
}
function validate63(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.key === void 0 && (missing0 = "key") || data.label === void 0 && (missing0 = "label") || data.param_type === void 0 && (missing0 = "param_type") || data.default === void 0 && (missing0 = "default") || data.description === void 0 && (missing0 = "description") || data.constraints === void 0 && (missing0 = "constraints")) {
        validate63.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "constraints" || key0 === "default" || key0 === "description" || key0 === "key" || key0 === "label" || key0 === "param_type")) {
            validate63.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
            if (!validate64(data0, { instancePath: instancePath + "/constraints", parentData: data, parentDataProperty: "constraints", rootData })) {
              vErrors = vErrors === null ? validate64.errors : vErrors.concat(validate64.errors);
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
              validate63.errors = vErrors;
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
              if (!validate65(data.default, { instancePath: instancePath + "/default", parentData: data, parentDataProperty: "default", rootData })) {
                vErrors = vErrors === null ? validate65.errors : vErrors.concat(validate65.errors);
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
                  validate63.errors = [{ instancePath: instancePath + "/description", schemaPath: "#/properties/description/type", keyword: "type", params: { type: schema73.properties.description.type }, message: "must be string,null" }];
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
                    validate63.errors = [{ instancePath: instancePath + "/key", schemaPath: "#/properties/key/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                      validate63.errors = [{ instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                        validate63.errors = [{ instancePath: instancePath + "/param_type", schemaPath: "#/definitions/ParamType/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                      if (!(data5 === "Number" || data5 === "Integer" || data5 === "String" || data5 === "Boolean")) {
                        validate63.errors = [{ instancePath: instancePath + "/param_type", schemaPath: "#/definitions/ParamType/enum", keyword: "enum", params: { allowedValues: schema76.enum }, message: "must be equal to one of the allowed values" }];
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
      validate63.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate63.errors = vErrors;
  return errors === 0;
}
function validate62(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.model_id === void 0 && (missing0 = "model_id") || data.inference_settings === void 0 && (missing0 = "inference_settings")) {
        validate62.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "inference_settings" || key0 === "model_id" || key0 === "success")) {
            validate62.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate63(data0[i0], { instancePath: instancePath + "/inference_settings/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate63.errors : vErrors.concat(validate63.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate62.errors = [{ instancePath: instancePath + "/inference_settings", schemaPath: "#/properties/inference_settings/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate62.errors = [{ instancePath: instancePath + "/model_id", schemaPath: "#/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                  validate62.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                  return false;
                }
                if (true !== data3) {
                  validate62.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate62.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate62.errors = vErrors;
  return errors === 0;
}
var validateInstallationProgressOutcome = validate70;
var schema78 = { "additionalProperties": false, "properties": { "completedAt": { "type": ["string", "null"] }, "completedDependencies": { "maximum": 4294967295, "minimum": 0, "type": "integer" }, "completedItems": { "items": { "$ref": "#/definitions/RuntimeInstallationProgressItem" }, "type": "array" }, "currentItem": { "type": ["string", "null"] }, "dependencyCount": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "downloadSpeed": { "maximum": 17976931348623157e292, "minimum": -17976931348623157e292, "type": ["number", "null"] }, "downloadedBytes": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" }, "error": { "type": ["string", "null"] }, "etaSeconds": { "maximum": 17976931348623157e292, "minimum": -17976931348623157e292, "type": ["number", "null"] }, "logPath": { "type": ["string", "null"] }, "overallProgress": { "maximum": 34028234663852886e22, "minimum": -34028234663852886e22, "type": ["number", "null"] }, "stage": { "$ref": "#/definitions/RuntimeInstallationStage" }, "stageProgress": { "maximum": 34028234663852886e22, "minimum": -34028234663852886e22, "type": ["number", "null"] }, "startedAt": { "type": "string" }, "success": { "type": ["boolean", "null"] }, "tag": { "type": "string" }, "totalSize": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] } }, "required": ["tag", "startedAt", "stage", "stageProgress", "overallProgress", "currentItem", "downloadSpeed", "etaSeconds", "totalSize", "downloadedBytes", "dependencyCount", "completedDependencies", "completedItems", "error", "completedAt", "success", "logPath"], "type": "object" };
var schema79 = { "additionalProperties": false, "properties": { "completedAt": { "type": "string" }, "name": { "type": "string" }, "size": { "maximum": 9007199254740991, "minimum": 0, "type": ["integer", "null"] }, "type": { "type": "string" } }, "required": ["name", "type", "size", "completedAt"], "type": "object" };
var schema80 = { "enum": ["download", "extract", "venv", "dependencies", "setup"], "type": "string" };
function validate71(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.tag === void 0 && (missing0 = "tag") || data.startedAt === void 0 && (missing0 = "startedAt") || data.stage === void 0 && (missing0 = "stage") || data.stageProgress === void 0 && (missing0 = "stageProgress") || data.overallProgress === void 0 && (missing0 = "overallProgress") || data.currentItem === void 0 && (missing0 = "currentItem") || data.downloadSpeed === void 0 && (missing0 = "downloadSpeed") || data.etaSeconds === void 0 && (missing0 = "etaSeconds") || data.totalSize === void 0 && (missing0 = "totalSize") || data.downloadedBytes === void 0 && (missing0 = "downloadedBytes") || data.dependencyCount === void 0 && (missing0 = "dependencyCount") || data.completedDependencies === void 0 && (missing0 = "completedDependencies") || data.completedItems === void 0 && (missing0 = "completedItems") || data.error === void 0 && (missing0 = "error") || data.completedAt === void 0 && (missing0 = "completedAt") || data.success === void 0 && (missing0 = "success") || data.logPath === void 0 && (missing0 = "logPath")) {
        validate71.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema78.properties, key0)) {
            validate71.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.completedAt !== void 0) {
            let data0 = data.completedAt;
            const _errs2 = errors;
            if (typeof data0 !== "string" && data0 !== null) {
              validate71.errors = [{ instancePath: instancePath + "/completedAt", schemaPath: "#/properties/completedAt/type", keyword: "type", params: { type: schema78.properties.completedAt.type }, message: "must be string,null" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.completedDependencies !== void 0) {
              let data1 = data.completedDependencies;
              const _errs4 = errors;
              if (!(typeof data1 == "number" && (!(data1 % 1) && !isNaN(data1)) && isFinite(data1))) {
                validate71.errors = [{ instancePath: instancePath + "/completedDependencies", schemaPath: "#/properties/completedDependencies/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 4294967295 || isNaN(data1)) {
                    validate71.errors = [{ instancePath: instancePath + "/completedDependencies", schemaPath: "#/properties/completedDependencies/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate71.errors = [{ instancePath: instancePath + "/completedDependencies", schemaPath: "#/properties/completedDependencies/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
              if (data.completedItems !== void 0) {
                let data2 = data.completedItems;
                const _errs6 = errors;
                if (errors === _errs6) {
                  if (Array.isArray(data2)) {
                    var valid1 = true;
                    const len0 = data2.length;
                    for (let i0 = 0; i0 < len0; i0++) {
                      let data3 = data2[i0];
                      const _errs8 = errors;
                      const _errs9 = errors;
                      if (errors === _errs9) {
                        if (data3 && typeof data3 == "object" && !Array.isArray(data3)) {
                          let missing1;
                          if (data3.name === void 0 && (missing1 = "name") || data3.type === void 0 && (missing1 = "type") || data3.size === void 0 && (missing1 = "size") || data3.completedAt === void 0 && (missing1 = "completedAt")) {
                            validate71.errors = [{ instancePath: instancePath + "/completedItems/" + i0, schemaPath: "#/definitions/RuntimeInstallationProgressItem/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" }];
                            return false;
                          } else {
                            const _errs11 = errors;
                            for (const key1 in data3) {
                              if (!(key1 === "completedAt" || key1 === "name" || key1 === "size" || key1 === "type")) {
                                validate71.errors = [{ instancePath: instancePath + "/completedItems/" + i0, schemaPath: "#/definitions/RuntimeInstallationProgressItem/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" }];
                                return false;
                                break;
                              }
                            }
                            if (_errs11 === errors) {
                              if (data3.completedAt !== void 0) {
                                const _errs12 = errors;
                                if (typeof data3.completedAt !== "string") {
                                  validate71.errors = [{ instancePath: instancePath + "/completedItems/" + i0 + "/completedAt", schemaPath: "#/definitions/RuntimeInstallationProgressItem/properties/completedAt/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                  return false;
                                }
                                var valid3 = _errs12 === errors;
                              } else {
                                var valid3 = true;
                              }
                              if (valid3) {
                                if (data3.name !== void 0) {
                                  const _errs14 = errors;
                                  if (typeof data3.name !== "string") {
                                    validate71.errors = [{ instancePath: instancePath + "/completedItems/" + i0 + "/name", schemaPath: "#/definitions/RuntimeInstallationProgressItem/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                    return false;
                                  }
                                  var valid3 = _errs14 === errors;
                                } else {
                                  var valid3 = true;
                                }
                                if (valid3) {
                                  if (data3.size !== void 0) {
                                    let data6 = data3.size;
                                    const _errs16 = errors;
                                    if (!(typeof data6 == "number" && (!(data6 % 1) && !isNaN(data6)) && isFinite(data6)) && data6 !== null) {
                                      validate71.errors = [{ instancePath: instancePath + "/completedItems/" + i0 + "/size", schemaPath: "#/definitions/RuntimeInstallationProgressItem/properties/size/type", keyword: "type", params: { type: schema79.properties.size.type }, message: "must be integer,null" }];
                                      return false;
                                    }
                                    if (errors === _errs16) {
                                      if (typeof data6 == "number" && isFinite(data6)) {
                                        if (data6 > 9007199254740991 || isNaN(data6)) {
                                          validate71.errors = [{ instancePath: instancePath + "/completedItems/" + i0 + "/size", schemaPath: "#/definitions/RuntimeInstallationProgressItem/properties/size/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                          return false;
                                        } else {
                                          if (data6 < 0 || isNaN(data6)) {
                                            validate71.errors = [{ instancePath: instancePath + "/completedItems/" + i0 + "/size", schemaPath: "#/definitions/RuntimeInstallationProgressItem/properties/size/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                            return false;
                                          }
                                        }
                                      }
                                    }
                                    var valid3 = _errs16 === errors;
                                  } else {
                                    var valid3 = true;
                                  }
                                  if (valid3) {
                                    if (data3.type !== void 0) {
                                      const _errs18 = errors;
                                      if (typeof data3.type !== "string") {
                                        validate71.errors = [{ instancePath: instancePath + "/completedItems/" + i0 + "/type", schemaPath: "#/definitions/RuntimeInstallationProgressItem/properties/type/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                        return false;
                                      }
                                      var valid3 = _errs18 === errors;
                                    } else {
                                      var valid3 = true;
                                    }
                                  }
                                }
                              }
                            }
                          }
                        } else {
                          validate71.errors = [{ instancePath: instancePath + "/completedItems/" + i0, schemaPath: "#/definitions/RuntimeInstallationProgressItem/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                          return false;
                        }
                      }
                      var valid1 = _errs8 === errors;
                      if (!valid1) {
                        break;
                      }
                    }
                  } else {
                    validate71.errors = [{ instancePath: instancePath + "/completedItems", schemaPath: "#/properties/completedItems/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.currentItem !== void 0) {
                  let data8 = data.currentItem;
                  const _errs20 = errors;
                  if (typeof data8 !== "string" && data8 !== null) {
                    validate71.errors = [{ instancePath: instancePath + "/currentItem", schemaPath: "#/properties/currentItem/type", keyword: "type", params: { type: schema78.properties.currentItem.type }, message: "must be string,null" }];
                    return false;
                  }
                  var valid0 = _errs20 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.dependencyCount !== void 0) {
                    let data9 = data.dependencyCount;
                    const _errs22 = errors;
                    if (!(typeof data9 == "number" && (!(data9 % 1) && !isNaN(data9)) && isFinite(data9)) && data9 !== null) {
                      validate71.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/type", keyword: "type", params: { type: schema78.properties.dependencyCount.type }, message: "must be integer,null" }];
                      return false;
                    }
                    if (errors === _errs22) {
                      if (typeof data9 == "number" && isFinite(data9)) {
                        if (data9 > 4294967295 || isNaN(data9)) {
                          validate71.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                          return false;
                        } else {
                          if (data9 < 0 || isNaN(data9)) {
                            validate71.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                    if (data.downloadSpeed !== void 0) {
                      let data10 = data.downloadSpeed;
                      const _errs24 = errors;
                      if (!(typeof data10 == "number" && isFinite(data10)) && data10 !== null) {
                        validate71.errors = [{ instancePath: instancePath + "/downloadSpeed", schemaPath: "#/properties/downloadSpeed/type", keyword: "type", params: { type: schema78.properties.downloadSpeed.type }, message: "must be number,null" }];
                        return false;
                      }
                      if (errors === _errs24) {
                        if (typeof data10 == "number" && isFinite(data10)) {
                          if (data10 > 17976931348623157e292 || isNaN(data10)) {
                            validate71.errors = [{ instancePath: instancePath + "/downloadSpeed", schemaPath: "#/properties/downloadSpeed/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                            return false;
                          } else {
                            if (data10 < -17976931348623157e292 || isNaN(data10)) {
                              validate71.errors = [{ instancePath: instancePath + "/downloadSpeed", schemaPath: "#/properties/downloadSpeed/minimum", keyword: "minimum", params: { comparison: ">=", limit: -17976931348623157e292 }, message: "must be >= -1.7976931348623157e+308" }];
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
                      if (data.downloadedBytes !== void 0) {
                        let data11 = data.downloadedBytes;
                        const _errs26 = errors;
                        if (!(typeof data11 == "number" && (!(data11 % 1) && !isNaN(data11)) && isFinite(data11))) {
                          validate71.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                          return false;
                        }
                        if (errors === _errs26) {
                          if (typeof data11 == "number" && isFinite(data11)) {
                            if (data11 > 9007199254740991 || isNaN(data11)) {
                              validate71.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                              return false;
                            } else {
                              if (data11 < 0 || isNaN(data11)) {
                                validate71.errors = [{ instancePath: instancePath + "/downloadedBytes", schemaPath: "#/properties/downloadedBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                return false;
                              }
                            }
                          }
                        }
                        var valid0 = _errs26 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.error !== void 0) {
                          let data12 = data.error;
                          const _errs28 = errors;
                          if (typeof data12 !== "string" && data12 !== null) {
                            validate71.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema78.properties.error.type }, message: "must be string,null" }];
                            return false;
                          }
                          var valid0 = _errs28 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.etaSeconds !== void 0) {
                            let data13 = data.etaSeconds;
                            const _errs30 = errors;
                            if (!(typeof data13 == "number" && isFinite(data13)) && data13 !== null) {
                              validate71.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/type", keyword: "type", params: { type: schema78.properties.etaSeconds.type }, message: "must be number,null" }];
                              return false;
                            }
                            if (errors === _errs30) {
                              if (typeof data13 == "number" && isFinite(data13)) {
                                if (data13 > 17976931348623157e292 || isNaN(data13)) {
                                  validate71.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                                  return false;
                                } else {
                                  if (data13 < -17976931348623157e292 || isNaN(data13)) {
                                    validate71.errors = [{ instancePath: instancePath + "/etaSeconds", schemaPath: "#/properties/etaSeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: -17976931348623157e292 }, message: "must be >= -1.7976931348623157e+308" }];
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
                            if (data.logPath !== void 0) {
                              let data14 = data.logPath;
                              const _errs32 = errors;
                              if (typeof data14 !== "string" && data14 !== null) {
                                validate71.errors = [{ instancePath: instancePath + "/logPath", schemaPath: "#/properties/logPath/type", keyword: "type", params: { type: schema78.properties.logPath.type }, message: "must be string,null" }];
                                return false;
                              }
                              var valid0 = _errs32 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.overallProgress !== void 0) {
                                let data15 = data.overallProgress;
                                const _errs34 = errors;
                                if (!(typeof data15 == "number" && isFinite(data15)) && data15 !== null) {
                                  validate71.errors = [{ instancePath: instancePath + "/overallProgress", schemaPath: "#/properties/overallProgress/type", keyword: "type", params: { type: schema78.properties.overallProgress.type }, message: "must be number,null" }];
                                  return false;
                                }
                                if (errors === _errs34) {
                                  if (typeof data15 == "number" && isFinite(data15)) {
                                    if (data15 > 34028234663852886e22 || isNaN(data15)) {
                                      validate71.errors = [{ instancePath: instancePath + "/overallProgress", schemaPath: "#/properties/overallProgress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 34028234663852886e22 }, message: "must be <= 3.4028234663852886e+38" }];
                                      return false;
                                    } else {
                                      if (data15 < -34028234663852886e22 || isNaN(data15)) {
                                        validate71.errors = [{ instancePath: instancePath + "/overallProgress", schemaPath: "#/properties/overallProgress/minimum", keyword: "minimum", params: { comparison: ">=", limit: -34028234663852886e22 }, message: "must be >= -3.4028234663852886e+38" }];
                                        return false;
                                      }
                                    }
                                  }
                                }
                                var valid0 = _errs34 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.stage !== void 0) {
                                  let data16 = data.stage;
                                  const _errs36 = errors;
                                  if (typeof data16 !== "string") {
                                    validate71.errors = [{ instancePath: instancePath + "/stage", schemaPath: "#/definitions/RuntimeInstallationStage/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                    return false;
                                  }
                                  if (!(data16 === "download" || data16 === "extract" || data16 === "venv" || data16 === "dependencies" || data16 === "setup")) {
                                    validate71.errors = [{ instancePath: instancePath + "/stage", schemaPath: "#/definitions/RuntimeInstallationStage/enum", keyword: "enum", params: { allowedValues: schema80.enum }, message: "must be equal to one of the allowed values" }];
                                    return false;
                                  }
                                  var valid0 = _errs36 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.stageProgress !== void 0) {
                                    let data17 = data.stageProgress;
                                    const _errs39 = errors;
                                    if (!(typeof data17 == "number" && isFinite(data17)) && data17 !== null) {
                                      validate71.errors = [{ instancePath: instancePath + "/stageProgress", schemaPath: "#/properties/stageProgress/type", keyword: "type", params: { type: schema78.properties.stageProgress.type }, message: "must be number,null" }];
                                      return false;
                                    }
                                    if (errors === _errs39) {
                                      if (typeof data17 == "number" && isFinite(data17)) {
                                        if (data17 > 34028234663852886e22 || isNaN(data17)) {
                                          validate71.errors = [{ instancePath: instancePath + "/stageProgress", schemaPath: "#/properties/stageProgress/maximum", keyword: "maximum", params: { comparison: "<=", limit: 34028234663852886e22 }, message: "must be <= 3.4028234663852886e+38" }];
                                          return false;
                                        } else {
                                          if (data17 < -34028234663852886e22 || isNaN(data17)) {
                                            validate71.errors = [{ instancePath: instancePath + "/stageProgress", schemaPath: "#/properties/stageProgress/minimum", keyword: "minimum", params: { comparison: ">=", limit: -34028234663852886e22 }, message: "must be >= -3.4028234663852886e+38" }];
                                            return false;
                                          }
                                        }
                                      }
                                    }
                                    var valid0 = _errs39 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.startedAt !== void 0) {
                                      const _errs41 = errors;
                                      if (typeof data.startedAt !== "string") {
                                        validate71.errors = [{ instancePath: instancePath + "/startedAt", schemaPath: "#/properties/startedAt/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                        return false;
                                      }
                                      var valid0 = _errs41 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                    if (valid0) {
                                      if (data.success !== void 0) {
                                        let data19 = data.success;
                                        const _errs43 = errors;
                                        if (typeof data19 !== "boolean" && data19 !== null) {
                                          validate71.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: schema78.properties.success.type }, message: "must be boolean,null" }];
                                          return false;
                                        }
                                        var valid0 = _errs43 === errors;
                                      } else {
                                        var valid0 = true;
                                      }
                                      if (valid0) {
                                        if (data.tag !== void 0) {
                                          const _errs45 = errors;
                                          if (typeof data.tag !== "string") {
                                            validate71.errors = [{ instancePath: instancePath + "/tag", schemaPath: "#/properties/tag/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                            return false;
                                          }
                                          var valid0 = _errs45 === errors;
                                        } else {
                                          var valid0 = true;
                                        }
                                        if (valid0) {
                                          if (data.totalSize !== void 0) {
                                            let data21 = data.totalSize;
                                            const _errs47 = errors;
                                            if (!(typeof data21 == "number" && (!(data21 % 1) && !isNaN(data21)) && isFinite(data21)) && data21 !== null) {
                                              validate71.errors = [{ instancePath: instancePath + "/totalSize", schemaPath: "#/properties/totalSize/type", keyword: "type", params: { type: schema78.properties.totalSize.type }, message: "must be integer,null" }];
                                              return false;
                                            }
                                            if (errors === _errs47) {
                                              if (typeof data21 == "number" && isFinite(data21)) {
                                                if (data21 > 9007199254740991 || isNaN(data21)) {
                                                  validate71.errors = [{ instancePath: instancePath + "/totalSize", schemaPath: "#/properties/totalSize/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                                  return false;
                                                } else {
                                                  if (data21 < 0 || isNaN(data21)) {
                                                    validate71.errors = [{ instancePath: instancePath + "/totalSize", schemaPath: "#/properties/totalSize/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                    return false;
                                                  }
                                                }
                                              }
                                            }
                                            var valid0 = _errs47 === errors;
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
      validate71.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate71.errors = vErrors;
  return errors === 0;
}
function validate70(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (!validate71(data, { instancePath, parentData, parentDataProperty, rootData })) {
    vErrors = vErrors === null ? validate71.errors : vErrors.concat(validate71.errors);
    errors = vErrors.length;
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs2 = errors;
    if (data !== null) {
      const err0 = { instancePath, schemaPath: "#/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    var _valid0 = _errs2 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err1 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
    validate70.errors = vErrors;
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
  validate70.errors = vErrors;
  return errors === 0;
}
var validateInstalledVersionsOutcome = validate73;
function validate73(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.versions === void 0 && (missing0 = "versions")) {
        validate73.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "success" || key0 === "versions")) {
            validate73.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.success !== void 0) {
            let data0 = data.success;
            const _errs2 = errors;
            if (typeof data0 !== "boolean") {
              validate73.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
            }
            if (true !== data0) {
              validate73.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.versions !== void 0) {
              let data1 = data.versions;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (Array.isArray(data1)) {
                  var valid1 = true;
                  const len0 = data1.length;
                  for (let i0 = 0; i0 < len0; i0++) {
                    const _errs6 = errors;
                    if (typeof data1[i0] !== "string") {
                      validate73.errors = [{ instancePath: instancePath + "/versions/" + i0, schemaPath: "#/properties/versions/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                      return false;
                    }
                    var valid1 = _errs6 === errors;
                    if (!valid1) {
                      break;
                    }
                  }
                } else {
                  validate73.errors = [{ instancePath: instancePath + "/versions", schemaPath: "#/properties/versions/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
      validate73.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate73.errors = vErrors;
  return errors === 0;
}
var validateInstallVersionOutcome = validate74;
function validate74(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  const _errs2 = errors;
  if (errors === _errs2) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.message === void 0 && (missing0 = "message")) {
        const err0 = { instancePath, schemaPath: "#/definitions/InstallVersionStarted/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs4 = errors;
        for (const key0 in data) {
          if (!(key0 === "message" || key0 === "success")) {
            const err1 = { instancePath, schemaPath: "#/definitions/InstallVersionStarted/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
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
          if (data.message !== void 0) {
            const _errs5 = errors;
            if (typeof data.message !== "string") {
              const err2 = { instancePath: instancePath + "/message", schemaPath: "#/definitions/InstallVersionStarted/properties/message/type", keyword: "type", params: { type: "string" }, message: "must be string" };
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
            if (data.success !== void 0) {
              let data1 = data.success;
              const _errs7 = errors;
              if (typeof data1 !== "boolean") {
                const err3 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/InstallVersionStarted/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                if (vErrors === null) {
                  vErrors = [err3];
                } else {
                  vErrors.push(err3);
                }
                errors++;
              }
              if (true !== data1) {
                const err4 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/InstallVersionStarted/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err4];
                } else {
                  vErrors.push(err4);
                }
                errors++;
              }
              var valid2 = _errs7 === errors;
            } else {
              var valid2 = true;
            }
          }
        }
      }
    } else {
      const err5 = { instancePath, schemaPath: "#/definitions/InstallVersionStarted/type", keyword: "type", params: { type: "object" }, message: "must be object" };
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
    const _errs9 = errors;
    const _errs10 = errors;
    if (errors === _errs10) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.success === void 0 && (missing1 = "success") || data.error === void 0 && (missing1 = "error")) {
          const err6 = { instancePath, schemaPath: "#/definitions/InstallVersionFailed/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err6];
          } else {
            vErrors.push(err6);
          }
          errors++;
        } else {
          const _errs12 = errors;
          for (const key1 in data) {
            if (!(key1 === "error" || key1 === "success")) {
              const err7 = { instancePath, schemaPath: "#/definitions/InstallVersionFailed/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
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
            if (data.error !== void 0) {
              const _errs13 = errors;
              if (typeof data.error !== "string") {
                const err8 = { instancePath: instancePath + "/error", schemaPath: "#/definitions/InstallVersionFailed/properties/error/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err8];
                } else {
                  vErrors.push(err8);
                }
                errors++;
              }
              var valid4 = _errs13 === errors;
            } else {
              var valid4 = true;
            }
            if (valid4) {
              if (data.success !== void 0) {
                let data3 = data.success;
                const _errs15 = errors;
                if (typeof data3 !== "boolean") {
                  const err9 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/InstallVersionFailed/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                  if (vErrors === null) {
                    vErrors = [err9];
                  } else {
                    vErrors.push(err9);
                  }
                  errors++;
                }
                if (false !== data3) {
                  const err10 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/InstallVersionFailed/properties/success/const", keyword: "const", params: { allowedValue: false }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err10];
                  } else {
                    vErrors.push(err10);
                  }
                  errors++;
                }
                var valid4 = _errs15 === errors;
              } else {
                var valid4 = true;
              }
            }
          }
        }
      } else {
        const err11 = { instancePath, schemaPath: "#/definitions/InstallVersionFailed/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
    }
    var _valid0 = _errs9 === errors;
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
    validate74.errors = vErrors;
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
  validate74.errors = vErrors;
  return errors === 0;
}
var validateInstallVersionParams = validate75;
function validate75(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.app_id === void 0 && (missing0 = "app_id") || data.tag === void 0 && (missing0 = "tag")) {
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
          if (!(key0 === "app_id" || key0 === "tag")) {
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
          if (data.app_id !== void 0) {
            const _errs4 = errors;
            if (typeof data.app_id !== "string") {
              const err2 = { instancePath: instancePath + "/app_id", schemaPath: "#/anyOf/0/properties/app_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.tag !== void 0) {
              const _errs6 = errors;
              if (typeof data.tag !== "string") {
                const err3 = { instancePath: instancePath + "/tag", schemaPath: "#/anyOf/0/properties/tag/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err3];
                } else {
                  vErrors.push(err3);
                }
                errors++;
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      const err4 = { instancePath, schemaPath: "#/anyOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs8 = errors;
    if (errors === _errs8) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.appId === void 0 && (missing1 = "appId") || data.tag === void 0 && (missing1 = "tag")) {
          const err5 = { instancePath, schemaPath: "#/anyOf/1/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        } else {
          const _errs10 = errors;
          for (const key1 in data) {
            if (!(key1 === "appId" || key1 === "tag")) {
              const err6 = { instancePath, schemaPath: "#/anyOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err6];
              } else {
                vErrors.push(err6);
              }
              errors++;
              break;
            }
          }
          if (_errs10 === errors) {
            if (data.appId !== void 0) {
              const _errs11 = errors;
              if (typeof data.appId !== "string") {
                const err7 = { instancePath: instancePath + "/appId", schemaPath: "#/anyOf/1/properties/appId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err7];
                } else {
                  vErrors.push(err7);
                }
                errors++;
              }
              var valid2 = _errs11 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.tag !== void 0) {
                const _errs13 = errors;
                if (typeof data.tag !== "string") {
                  const err8 = { instancePath: instancePath + "/tag", schemaPath: "#/anyOf/1/properties/tag/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err8];
                  } else {
                    vErrors.push(err8);
                  }
                  errors++;
                }
                var valid2 = _errs13 === errors;
              } else {
                var valid2 = true;
              }
            }
          }
        }
      } else {
        const err9 = { instancePath, schemaPath: "#/anyOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    var _valid0 = _errs8 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
    validate75.errors = vErrors;
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
  validate75.errors = vErrors;
  return errors === 0;
}
var validateLibraryModelMetadataOutcome = validate76;
var schema87 = { "additionalProperties": false, "description": "Derived component metadata for a directory-root bundle model.", "properties": { "class_name": { "default": null, "type": ["string", "null"] }, "name": { "type": "string" }, "relative_path": { "type": "string" }, "source_library": { "default": null, "type": ["string", "null"] }, "state": { "$ref": "#/definitions/BundleComponentState" } }, "required": ["name", "relative_path", "source_library", "class_name", "state"], "type": "object" };
var schema88 = { "description": "Presence/state of a bundle component derived from bundle metadata.", "enum": ["present", "missing", "unreadable", "path_escape"], "type": "string" };
function validate77(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.name === void 0 && (missing0 = "name") || data.relative_path === void 0 && (missing0 = "relative_path") || data.source_library === void 0 && (missing0 = "source_library") || data.class_name === void 0 && (missing0 = "class_name") || data.state === void 0 && (missing0 = "state")) {
        validate77.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "class_name" || key0 === "name" || key0 === "relative_path" || key0 === "source_library" || key0 === "state")) {
            validate77.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.class_name !== void 0) {
            let data0 = data.class_name;
            const _errs2 = errors;
            if (typeof data0 !== "string" && data0 !== null) {
              validate77.errors = [{ instancePath: instancePath + "/class_name", schemaPath: "#/properties/class_name/type", keyword: "type", params: { type: schema87.properties.class_name.type }, message: "must be string,null" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.name !== void 0) {
              const _errs4 = errors;
              if (typeof data.name !== "string") {
                validate77.errors = [{ instancePath: instancePath + "/name", schemaPath: "#/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.relative_path !== void 0) {
                const _errs6 = errors;
                if (typeof data.relative_path !== "string") {
                  validate77.errors = [{ instancePath: instancePath + "/relative_path", schemaPath: "#/properties/relative_path/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                  return false;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.source_library !== void 0) {
                  let data3 = data.source_library;
                  const _errs8 = errors;
                  if (typeof data3 !== "string" && data3 !== null) {
                    validate77.errors = [{ instancePath: instancePath + "/source_library", schemaPath: "#/properties/source_library/type", keyword: "type", params: { type: schema87.properties.source_library.type }, message: "must be string,null" }];
                    return false;
                  }
                  var valid0 = _errs8 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.state !== void 0) {
                    let data4 = data.state;
                    const _errs10 = errors;
                    if (typeof data4 !== "string") {
                      validate77.errors = [{ instancePath: instancePath + "/state", schemaPath: "#/definitions/BundleComponentState/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                      return false;
                    }
                    if (!(data4 === "present" || data4 === "missing" || data4 === "unreadable" || data4 === "path_escape")) {
                      validate77.errors = [{ instancePath: instancePath + "/state", schemaPath: "#/definitions/BundleComponentState/enum", keyword: "enum", params: { allowedValues: schema88.enum }, message: "must be equal to one of the allowed values" }];
                      return false;
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate77.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate77.errors = vErrors;
  return errors === 0;
}
var wrapper2 = { validate: validate79 };
function validate79(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
                if (!wrapper2.validate(data[i0], { instancePath: instancePath + "/" + i0, parentData: data, parentDataProperty: i0, rootData })) {
                  vErrors = vErrors === null ? wrapper2.validate.errors : vErrors.concat(wrapper2.validate.errors);
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
                  if (!wrapper2.validate(data[key0], { instancePath: instancePath + "/" + key0.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data, parentDataProperty: key0, rootData })) {
                    vErrors = vErrors === null ? wrapper2.validate.errors : vErrors.concat(wrapper2.validate.errors);
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
    validate79.errors = vErrors;
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
  validate79.errors = vErrors;
  return errors === 0;
}
function validate81(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.file_type === void 0 && (missing0 = "file_type") || data.metadata === void 0 && (missing0 = "metadata")) {
        validate81.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "file_type" || key0 === "metadata")) {
            validate81.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.file_type !== void 0) {
            const _errs2 = errors;
            if (typeof data.file_type !== "string") {
              validate81.errors = [{ instancePath: instancePath + "/file_type", schemaPath: "#/properties/file_type/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.metadata !== void 0) {
              let data1 = data.metadata;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                  for (const key1 in data1) {
                    const _errs7 = errors;
                    if (!validate79(data1[key1], { instancePath: instancePath + "/metadata/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data1, parentDataProperty: key1, rootData })) {
                      vErrors = vErrors === null ? validate79.errors : vErrors.concat(validate79.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs7 === errors;
                    if (!valid1) {
                      break;
                    }
                  }
                } else {
                  validate81.errors = [{ instancePath: instancePath + "/metadata", schemaPath: "#/properties/metadata/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
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
      validate81.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate81.errors = vErrors;
  return errors === 0;
}
function validate76(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.model_id === void 0 && (missing0 = "model_id")) {
        validate76.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "component_manifest" || key0 === "effective_metadata" || key0 === "embedded_metadata" || key0 === "model_id" || key0 === "primary_file" || key0 === "stored_metadata" || key0 === "success")) {
            validate76.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.component_manifest !== void 0) {
            let data0 = data.component_manifest;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (!validate77(data0[i0], { instancePath: instancePath + "/component_manifest/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate77.errors : vErrors.concat(validate77.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate76.errors = [{ instancePath: instancePath + "/component_manifest", schemaPath: "#/properties/component_manifest/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.effective_metadata !== void 0) {
              let data2 = data.effective_metadata;
              const _errs5 = errors;
              if (errors === _errs5) {
                if (data2 && typeof data2 == "object" && !Array.isArray(data2)) {
                  for (const key1 in data2) {
                    const _errs8 = errors;
                    if (!validate79(data2[key1], { instancePath: instancePath + "/effective_metadata/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data2, parentDataProperty: key1, rootData })) {
                      vErrors = vErrors === null ? validate79.errors : vErrors.concat(validate79.errors);
                      errors = vErrors.length;
                    }
                    var valid2 = _errs8 === errors;
                    if (!valid2) {
                      break;
                    }
                  }
                } else {
                  validate76.errors = [{ instancePath: instancePath + "/effective_metadata", schemaPath: "#/properties/effective_metadata/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                  return false;
                }
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.embedded_metadata !== void 0) {
                const _errs9 = errors;
                if (!validate81(data.embedded_metadata, { instancePath: instancePath + "/embedded_metadata", parentData: data, parentDataProperty: "embedded_metadata", rootData })) {
                  vErrors = vErrors === null ? validate81.errors : vErrors.concat(validate81.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs9 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.model_id !== void 0) {
                  const _errs10 = errors;
                  if (typeof data.model_id !== "string") {
                    validate76.errors = [{ instancePath: instancePath + "/model_id", schemaPath: "#/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                  var valid0 = _errs10 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.primary_file !== void 0) {
                    const _errs12 = errors;
                    if (typeof data.primary_file !== "string") {
                      validate76.errors = [{ instancePath: instancePath + "/primary_file", schemaPath: "#/properties/primary_file/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                      return false;
                    }
                    var valid0 = _errs12 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.stored_metadata !== void 0) {
                      let data7 = data.stored_metadata;
                      const _errs14 = errors;
                      if (errors === _errs14) {
                        if (data7 && typeof data7 == "object" && !Array.isArray(data7)) {
                          for (const key2 in data7) {
                            const _errs17 = errors;
                            if (!validate79(data7[key2], { instancePath: instancePath + "/stored_metadata/" + key2.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data7, parentDataProperty: key2, rootData })) {
                              vErrors = vErrors === null ? validate79.errors : vErrors.concat(validate79.errors);
                              errors = vErrors.length;
                            }
                            var valid3 = _errs17 === errors;
                            if (!valid3) {
                              break;
                            }
                          }
                        } else {
                          validate76.errors = [{ instancePath: instancePath + "/stored_metadata", schemaPath: "#/properties/stored_metadata/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                          return false;
                        }
                      }
                      var valid0 = _errs14 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.success !== void 0) {
                        let data9 = data.success;
                        const _errs18 = errors;
                        if (typeof data9 !== "boolean") {
                          validate76.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                          return false;
                        }
                        if (true !== data9) {
                          validate76.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                          return false;
                        }
                        var valid0 = _errs18 === errors;
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
    } else {
      validate76.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate76.errors = vErrors;
  return errors === 0;
}
var validateLinkHealthOutcome = validate85;
var schema92 = { "additionalProperties": false, "description": "Link health response.\n\nNote: Not FFI-compatible due to `usize` fields. Use wrapper types in pumas-uniffi.", "properties": { "broken_links": { "items": { "type": "string" }, "type": "array" }, "error": { "type": "null" }, "errors": { "items": { "type": "string" }, "type": "array" }, "healthy_links": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" }, "orphaned_links": { "items": { "type": "string" }, "type": "array" }, "status": { "enum": ["healthy", "degraded"], "type": "string" }, "success": { "const": true, "type": "boolean" }, "total_links": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" }, "warnings": { "items": { "type": "string" }, "type": "array" } }, "pumasLinkHealth": true, "required": ["success", "status", "total_links", "healthy_links", "broken_links", "orphaned_links", "warnings", "errors"], "type": "object" };
function validate85(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.status === void 0 && (missing0 = "status") || data.total_links === void 0 && (missing0 = "total_links") || data.healthy_links === void 0 && (missing0 = "healthy_links") || data.broken_links === void 0 && (missing0 = "broken_links") || data.orphaned_links === void 0 && (missing0 = "orphaned_links") || data.warnings === void 0 && (missing0 = "warnings") || data.errors === void 0 && (missing0 = "errors")) {
        validate85.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (!func2.call(schema92.properties, key0)) {
            validate85.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                    validate85.errors = [{ instancePath: instancePath + "/broken_links/" + i0, schemaPath: "#/definitions/LinkHealthResponse/properties/broken_links/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                  var valid3 = _errs6 === errors;
                  if (!valid3) {
                    break;
                  }
                }
              } else {
                validate85.errors = [{ instancePath: instancePath + "/broken_links", schemaPath: "#/definitions/LinkHealthResponse/properties/broken_links/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate85.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/definitions/LinkHealthResponse/properties/error/type", keyword: "type", params: { type: "null" }, message: "must be null" }];
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
                        validate85.errors = [{ instancePath: instancePath + "/errors/" + i1, schemaPath: "#/definitions/LinkHealthResponse/properties/errors/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                      var valid4 = _errs12 === errors;
                      if (!valid4) {
                        break;
                      }
                    }
                  } else {
                    validate85.errors = [{ instancePath: instancePath + "/errors", schemaPath: "#/definitions/LinkHealthResponse/properties/errors/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                    validate85.errors = [{ instancePath: instancePath + "/healthy_links", schemaPath: "#/definitions/LinkHealthResponse/properties/healthy_links/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                    return false;
                  }
                  if (errors === _errs14) {
                    if (typeof data5 == "number" && isFinite(data5)) {
                      if (data5 > 9007199254740991 || isNaN(data5)) {
                        validate85.errors = [{ instancePath: instancePath + "/healthy_links", schemaPath: "#/definitions/LinkHealthResponse/properties/healthy_links/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                        return false;
                      } else {
                        if (data5 < 0 || isNaN(data5)) {
                          validate85.errors = [{ instancePath: instancePath + "/healthy_links", schemaPath: "#/definitions/LinkHealthResponse/properties/healthy_links/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                            validate85.errors = [{ instancePath: instancePath + "/orphaned_links/" + i2, schemaPath: "#/definitions/LinkHealthResponse/properties/orphaned_links/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                            return false;
                          }
                          var valid5 = _errs18 === errors;
                          if (!valid5) {
                            break;
                          }
                        }
                      } else {
                        validate85.errors = [{ instancePath: instancePath + "/orphaned_links", schemaPath: "#/definitions/LinkHealthResponse/properties/orphaned_links/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                        validate85.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/LinkHealthResponse/properties/status/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                      if (!(data8 === "healthy" || data8 === "degraded")) {
                        validate85.errors = [{ instancePath: instancePath + "/status", schemaPath: "#/definitions/LinkHealthResponse/properties/status/enum", keyword: "enum", params: { allowedValues: schema92.properties.status.enum }, message: "must be equal to one of the allowed values" }];
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
                          validate85.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/definitions/LinkHealthResponse/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                          return false;
                        }
                        if (true !== data9) {
                          validate85.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/definitions/LinkHealthResponse/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
                            validate85.errors = [{ instancePath: instancePath + "/total_links", schemaPath: "#/definitions/LinkHealthResponse/properties/total_links/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                            return false;
                          }
                          if (errors === _errs24) {
                            if (typeof data10 == "number" && isFinite(data10)) {
                              if (data10 > 9007199254740991 || isNaN(data10)) {
                                validate85.errors = [{ instancePath: instancePath + "/total_links", schemaPath: "#/definitions/LinkHealthResponse/properties/total_links/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                return false;
                              } else {
                                if (data10 < 0 || isNaN(data10)) {
                                  validate85.errors = [{ instancePath: instancePath + "/total_links", schemaPath: "#/definitions/LinkHealthResponse/properties/total_links/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate85.errors = [{ instancePath: instancePath + "/warnings/" + i3, schemaPath: "#/definitions/LinkHealthResponse/properties/warnings/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                    return false;
                                  }
                                  var valid6 = _errs28 === errors;
                                  if (!valid6) {
                                    break;
                                  }
                                }
                              } else {
                                validate85.errors = [{ instancePath: instancePath + "/warnings", schemaPath: "#/definitions/LinkHealthResponse/properties/warnings/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                return false;
                              }
                            }
                            var valid2 = _errs26 === errors;
                          } else {
                            var valid2 = true;
                          }
                          if (valid2) {
                            if (!Array.isArray(data.broken_links) || data.healthy_links + data.broken_links.length !== data.total_links || data.status === "healthy" !== (data.broken_links.length === 0)) {
                              validate85.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/pumasLinkHealth", keyword: "pumasLinkHealth", params: {}, message: 'must pass "pumasLinkHealth" keyword validation' }];
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
      validate85.errors = [{ instancePath, schemaPath: "#/definitions/LinkHealthResponse/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (!Array.isArray(data.broken_links) || data.healthy_links + data.broken_links.length !== data.total_links || data.status === "healthy" !== (data.broken_links.length === 0)) {
        validate85.errors = [{ instancePath, schemaPath: "#/pumasLinkHealth", keyword: "pumasLinkHealth", params: {}, message: 'must pass "pumasLinkHealth" keyword validation' }];
        return false;
      }
    }
  }
  validate85.errors = vErrors;
  return errors === 0;
}
var validateModelIndexRefreshOutcome = validate86;
function validate86(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.indexed_count === void 0 && (missing0 = "indexed_count")) {
        validate86.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "indexed_count" || key0 === "success")) {
            validate86.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.indexed_count !== void 0) {
            let data0 = data.indexed_count;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0))) {
              validate86.errors = [{ instancePath: instancePath + "/indexed_count", schemaPath: "#/properties/indexed_count/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 4294967295 || isNaN(data0)) {
                  validate86.errors = [{ instancePath: instancePath + "/indexed_count", schemaPath: "#/properties/indexed_count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                  return false;
                } else {
                  if (data0 < 0 || isNaN(data0)) {
                    validate86.errors = [{ instancePath: instancePath + "/indexed_count", schemaPath: "#/properties/indexed_count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                validate86.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate86.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate86.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate86.errors = vErrors;
  return errors === 0;
}
var validateModelsOutcome = validate87;
var schema95 = { "additionalProperties": false, "properties": { "artifact": { "$ref": "#/definitions/CatalogArtifactState" }, "dependencyCount": { "maximum": 512, "minimum": 0, "type": "integer" }, "displayDate": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "displayName": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "format": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "id": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "integrity": { "$ref": "#/definitions/CatalogIntegrityState" }, "modelDir": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "modelType": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "quantization": { "minLength": 1, "pumasCanonicalText": true, "pumasUtf8Max": 4096, "type": "string" }, "relatedAvailable": { "type": "boolean" }, "sizeBytes": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" } }, "pumasCatalogRow": true, "required": ["id", "modelDir", "displayName", "modelType", "dependencyCount", "relatedAvailable", "artifact", "integrity"], "type": "object" };
var schema97 = { "enum": ["part_file_present", "expected_files_missing"], "type": "string" };
var pattern17 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern18 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern19 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate89(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
                          const err14 = { instancePath: instancePath + "/reasons/" + i0, schemaPath: "#/definitions/CatalogPartialReason/enum", keyword: "enum", params: { allowedValues: schema97.enum }, message: "must be equal to one of the allowed values" };
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
                                if (func5(data6) > 96) {
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
    validate89.errors = vErrors;
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
  validate89.errors = vErrors;
  return errors === 0;
}
var pattern20 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern21 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern22 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern23 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern24 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern25 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
var pattern26 = new RegExp("^\\p{White_Space}|\\p{White_Space}$", "u");
function validate88(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.id === void 0 && (missing0 = "id") || data.modelDir === void 0 && (missing0 = "modelDir") || data.displayName === void 0 && (missing0 = "displayName") || data.modelType === void 0 && (missing0 = "modelType") || data.dependencyCount === void 0 && (missing0 = "dependencyCount") || data.relatedAvailable === void 0 && (missing0 = "relatedAvailable") || data.artifact === void 0 && (missing0 = "artifact") || data.integrity === void 0 && (missing0 = "integrity")) {
        validate88.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func2.call(schema95.properties, key0)) {
            validate88.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.artifact !== void 0) {
            const _errs2 = errors;
            if (!validate89(data.artifact, { instancePath: instancePath + "/artifact", parentData: data, parentDataProperty: "artifact", rootData })) {
              vErrors = vErrors === null ? validate89.errors : vErrors.concat(validate89.errors);
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
                validate88.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs3) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 512 || isNaN(data1)) {
                    validate88.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 512 }, message: "must be <= 512" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate88.errors = [{ instancePath: instancePath + "/dependencyCount", schemaPath: "#/properties/dependencyCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                    if (func5(data2) < 1) {
                      validate88.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                      return false;
                    } else {
                      if (data2.length === 0 || pattern20.test(data2)) {
                        validate88.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                        return false;
                      } else {
                        if (encodeURIComponent(data2).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                          validate88.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate88.errors = [{ instancePath: instancePath + "/displayDate", schemaPath: "#/properties/displayDate/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                      if (func5(data3) < 1) {
                        validate88.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                        return false;
                      } else {
                        if (data3.length === 0 || pattern21.test(data3)) {
                          validate88.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                          return false;
                        } else {
                          if (encodeURIComponent(data3).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                            validate88.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate88.errors = [{ instancePath: instancePath + "/displayName", schemaPath: "#/properties/displayName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                        if (func5(data4) < 1) {
                          validate88.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                          return false;
                        } else {
                          if (data4.length === 0 || pattern22.test(data4)) {
                            validate88.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                            return false;
                          } else {
                            if (encodeURIComponent(data4).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                              validate88.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate88.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                          if (func5(data5) < 1) {
                            validate88.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                            return false;
                          } else {
                            if (data5.length === 0 || pattern23.test(data5)) {
                              validate88.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                              return false;
                            } else {
                              if (encodeURIComponent(data5).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                validate88.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate88.errors = [{ instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                          validate88.errors = vErrors;
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
                              if (func5(data12) < 1) {
                                validate88.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                return false;
                              } else {
                                if (data12.length === 0 || pattern24.test(data12)) {
                                  validate88.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                  return false;
                                } else {
                                  if (encodeURIComponent(data12).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                    validate88.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                    return false;
                                  }
                                }
                              }
                            } else {
                              validate88.errors = [{ instancePath: instancePath + "/modelDir", schemaPath: "#/properties/modelDir/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                if (func5(data13) < 1) {
                                  validate88.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                  return false;
                                } else {
                                  if (data13.length === 0 || pattern25.test(data13)) {
                                    validate88.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                    return false;
                                  } else {
                                    if (encodeURIComponent(data13).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                      validate88.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                      return false;
                                    }
                                  }
                                }
                              } else {
                                validate88.errors = [{ instancePath: instancePath + "/modelType", schemaPath: "#/properties/modelType/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                  if (func5(data14) < 1) {
                                    validate88.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                    return false;
                                  } else {
                                    if (data14.length === 0 || pattern26.test(data14)) {
                                      validate88.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasCanonicalText", keyword: "pumasCanonicalText", params: {}, message: 'must pass "pumasCanonicalText" keyword validation' }];
                                      return false;
                                    } else {
                                      if (encodeURIComponent(data14).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                                        validate88.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                                        return false;
                                      }
                                    }
                                  }
                                } else {
                                  validate88.errors = [{ instancePath: instancePath + "/quantization", schemaPath: "#/properties/quantization/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                                  validate88.errors = [{ instancePath: instancePath + "/relatedAvailable", schemaPath: "#/properties/relatedAvailable/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
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
                                    validate88.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                    return false;
                                  }
                                  if (errors === _errs40) {
                                    if (typeof data16 == "number" && isFinite(data16)) {
                                      if (data16 > 9007199254740991 || isNaN(data16)) {
                                        validate88.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                                        return false;
                                      } else {
                                        if (data16 < 0 || isNaN(data16)) {
                                          validate88.errors = [{ instancePath: instancePath + "/sizeBytes", schemaPath: "#/properties/sizeBytes/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                                    validate88.errors = [{ instancePath, schemaPath: "#/pumasCatalogRow", keyword: "pumasCatalogRow", params: {}, message: 'must pass "pumasCatalogRow" keyword validation' }];
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
      validate88.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate88.errors = vErrors;
  return errors === 0;
}
function validate87(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.models === void 0 && (missing0 = "models")) {
        validate87.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "models" || key0 === "success")) {
            validate87.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate88(data0[key1], { instancePath: instancePath + "/models/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data0, parentDataProperty: key1, rootData })) {
                    vErrors = vErrors === null ? validate88.errors : vErrors.concat(validate88.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs5 === errors;
                  if (!valid1) {
                    break;
                  }
                }
                if (_errs4 === errors) {
                  if (Object.entries(data0).some(([key, value]) => value === null || typeof value !== "object" || key !== value.id)) {
                    validate87.errors = [{ instancePath: instancePath + "/models", schemaPath: "#/properties/models/pumasCatalogMap", keyword: "pumasCatalogMap", params: {}, message: 'must pass "pumasCatalogMap" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate87.errors = [{ instancePath: instancePath + "/models", schemaPath: "#/properties/models/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
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
                validate87.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate87.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate87.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate87.errors = vErrors;
  return errors === 0;
}
var validatePartialDownloadOutcome = validate92;
var schema100 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "definitions": { "DownloadStatus": { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" }, "PartialDownloadActionName": { "enum": ["resume", "recover", "attach", "none"], "type": "string" }, "PartialDownloadReason": { "enum": ["hf_client_unavailable", "download_root_busy", "model_not_found", "model_not_partial", "recovery_unavailable", "recovery_context_stale", "resume_rejected", "already_completed", "already_cancelled", "invalid_repo_id", "repo_not_found", "rate_limited", "permission_denied", "network_error", "recover_failed"], "type": "string" } }, "properties": { "action": { "$ref": "#/definitions/PartialDownloadActionName" }, "download_id": { "type": ["string", "null"] }, "error": { "type": ["string", "null"] }, "reason_code": { "anyOf": [{ "$ref": "#/definitions/PartialDownloadReason" }, { "type": "null" }] }, "status": { "anyOf": [{ "$ref": "#/definitions/DownloadStatus" }, { "type": "null" }] }, "success": { "type": "boolean" } }, "pumasPartialOutcome": true, "required": ["success", "action", "download_id", "status", "reason_code", "error"], "title": "PartialDownloadOutcome", "type": "object" };
var schema101 = { "enum": ["resume", "recover", "attach", "none"], "type": "string" };
var schema102 = { "enum": ["hf_client_unavailable", "download_root_busy", "model_not_found", "model_not_partial", "recovery_unavailable", "recovery_context_stale", "resume_rejected", "already_completed", "already_cancelled", "invalid_repo_id", "repo_not_found", "rate_limited", "permission_denied", "network_error", "recover_failed"], "type": "string" };
var schema103 = { "description": "Model download status.", "enum": ["queued", "downloading", "pausing", "paused", "cancelling", "completed", "cancelled", "error"], "type": "string" };
function validate92(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.action === void 0 && (missing0 = "action") || data.download_id === void 0 && (missing0 = "download_id") || data.status === void 0 && (missing0 = "status") || data.reason_code === void 0 && (missing0 = "reason_code") || data.error === void 0 && (missing0 = "error")) {
        validate92.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "action" || key0 === "download_id" || key0 === "error" || key0 === "reason_code" || key0 === "status" || key0 === "success")) {
            validate92.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.action !== void 0) {
            let data0 = data.action;
            const _errs2 = errors;
            if (typeof data0 !== "string") {
              validate92.errors = [{ instancePath: instancePath + "/action", schemaPath: "#/definitions/PartialDownloadActionName/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            if (!(data0 === "resume" || data0 === "recover" || data0 === "attach" || data0 === "none")) {
              validate92.errors = [{ instancePath: instancePath + "/action", schemaPath: "#/definitions/PartialDownloadActionName/enum", keyword: "enum", params: { allowedValues: schema101.enum }, message: "must be equal to one of the allowed values" }];
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
                validate92.errors = [{ instancePath: instancePath + "/download_id", schemaPath: "#/properties/download_id/type", keyword: "type", params: { type: schema100.properties.download_id.type }, message: "must be string,null" }];
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
                  validate92.errors = [{ instancePath: instancePath + "/error", schemaPath: "#/properties/error/type", keyword: "type", params: { type: schema100.properties.error.type }, message: "must be string,null" }];
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
                    const err1 = { instancePath: instancePath + "/reason_code", schemaPath: "#/definitions/PartialDownloadReason/enum", keyword: "enum", params: { allowedValues: schema102.enum }, message: "must be equal to one of the allowed values" };
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
                    validate92.errors = vErrors;
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
                      const err5 = { instancePath: instancePath + "/status", schemaPath: "#/definitions/DownloadStatus/enum", keyword: "enum", params: { allowedValues: schema103.enum }, message: "must be equal to one of the allowed values" };
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
                      validate92.errors = vErrors;
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
                        validate92.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                        return false;
                      }
                      var valid0 = _errs23 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (!((data.action === "resume" || data.action === "recover") && data.success === true && typeof data.download_id === "string" && data.download_id.length > 0 && data.status === "queued" && data.reason_code === null && data.error === null || data.action === "attach" && data.success === true && typeof data.download_id === "string" && data.download_id.length > 0 && ["queued", "downloading", "pausing", "cancelling"].includes(data.status) && data.reason_code === null && data.error === null || data.action === "none" && data.success === false && typeof data.error === "string" && (data.download_id === null && data.status === null && !["already_completed", "already_cancelled", "resume_rejected"].includes(data.reason_code) && data.reason_code !== null || typeof data.download_id === "string" && data.download_id.length > 0 && (data.status === "completed" && data.reason_code === "already_completed" || data.status === "cancelled" && data.reason_code === "already_cancelled" || ["paused", "error"].includes(data.status) && data.reason_code === "resume_rejected")))) {
                        validate92.errors = [{ instancePath, schemaPath: "#/pumasPartialOutcome", keyword: "pumasPartialOutcome", params: {}, message: 'must pass "pumasPartialOutcome" keyword validation' }];
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
      validate92.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate92.errors = vErrors;
  return errors === 0;
}
var validatePublicError = validate93;
var schema105 = { "description": "Stable public failure categories shared by RPC transports.", "enum": ["invalid_request", "not_found", "conflict", "cancelled", "unavailable", "operation_failed", "internal"], "type": "string" };
function validate93(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.code === void 0 && (missing0 = "code") || data.class === void 0 && (missing0 = "class") || data.message === void 0 && (missing0 = "message")) {
        validate93.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "class" || key0 === "code" || key0 === "message")) {
            validate93.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.class !== void 0) {
            let data0 = data.class;
            const _errs2 = errors;
            if (typeof data0 !== "string") {
              validate93.errors = [{ instancePath: instancePath + "/class", schemaPath: "#/definitions/PublicErrorClass/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
              return false;
            }
            if (!(data0 === "invalid_request" || data0 === "not_found" || data0 === "conflict" || data0 === "cancelled" || data0 === "unavailable" || data0 === "operation_failed" || data0 === "internal")) {
              validate93.errors = [{ instancePath: instancePath + "/class", schemaPath: "#/definitions/PublicErrorClass/enum", keyword: "enum", params: { allowedValues: schema105.enum }, message: "must be equal to one of the allowed values" }];
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
                validate93.errors = [{ instancePath: instancePath + "/code", schemaPath: "#/properties/code/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                return false;
              }
              if (errors === _errs5) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 2147483647 || isNaN(data1)) {
                    validate93.errors = [{ instancePath: instancePath + "/code", schemaPath: "#/properties/code/maximum", keyword: "maximum", params: { comparison: "<=", limit: 2147483647 }, message: "must be <= 2147483647" }];
                    return false;
                  } else {
                    if (data1 < -2147483648 || isNaN(data1)) {
                      validate93.errors = [{ instancePath: instancePath + "/code", schemaPath: "#/properties/code/minimum", keyword: "minimum", params: { comparison: ">=", limit: -2147483648 }, message: "must be >= -2147483648" }];
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
                  validate93.errors = [{ instancePath: instancePath + "/message", schemaPath: "#/properties/message/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
      validate93.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate93.errors = vErrors;
  return errors === 0;
}
var validateRecoverDownloadParams = validate94;
function validate94(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.modelId === void 0 && (missing0 = "modelId") || data.recoveryToken === void 0 && (missing0 = "recoveryToken")) {
        validate94.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "modelId" || key0 === "recoveryToken")) {
            validate94.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  validate94.errors = [{ instancePath: instancePath + "/modelId", schemaPath: "#/properties/modelId/pumasPortablePath", keyword: "pumasPortablePath", params: {}, message: 'must pass "pumasPortablePath" keyword validation' }];
                  return false;
                } else {
                  if (encodeURIComponent(data0).replace(/%[0-9A-F]{2}/g, "x").length > 4096) {
                    validate94.errors = [{ instancePath: instancePath + "/modelId", schemaPath: "#/properties/modelId/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                    return false;
                  }
                }
              } else {
                validate94.errors = [{ instancePath: instancePath + "/modelId", schemaPath: "#/properties/modelId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                    validate94.errors = [{ instancePath: instancePath + "/recoveryToken", schemaPath: "#/properties/recoveryToken/pattern", keyword: "pattern", params: { pattern: "^v1:[0-9a-f]{64}$" }, message: 'must match pattern "^v1:[0-9a-f]{64}$"' }];
                    return false;
                  }
                } else {
                  validate94.errors = [{ instancePath: instancePath + "/recoveryToken", schemaPath: "#/properties/recoveryToken/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
      validate94.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate94.errors = vErrors;
  return errors === 0;
}
var validateRemoveVersionOutcome = validate95;
function validate95(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success")) {
        validate95.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "success")) {
            validate95.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.success !== void 0) {
            if (typeof data.success !== "boolean") {
              validate95.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
            }
          }
        }
      }
    } else {
      validate95.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate95.errors = vErrors;
  return errors === 0;
}
var validateSearchCatalogParams = validate96;
var schema108 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "properties": { "limit": { "maximum": 512, "minimum": 1, "type": ["integer", "null"] }, "offset": { "maximum": 4294967295, "minimum": 0, "type": ["integer", "null"] }, "query": { "pumasUtf8Max": 4096, "type": "string" } }, "required": ["query"], "title": "SearchCatalogParams", "type": "object" };
function validate96(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.query === void 0 && (missing0 = "query")) {
        validate96.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "limit" || key0 === "offset" || key0 === "query")) {
            validate96.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.limit !== void 0) {
            let data0 = data.limit;
            const _errs2 = errors;
            if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0)) && data0 !== null) {
              validate96.errors = [{ instancePath: instancePath + "/limit", schemaPath: "#/properties/limit/type", keyword: "type", params: { type: schema108.properties.limit.type }, message: "must be integer,null" }];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 512 || isNaN(data0)) {
                  validate96.errors = [{ instancePath: instancePath + "/limit", schemaPath: "#/properties/limit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 512 }, message: "must be <= 512" }];
                  return false;
                } else {
                  if (data0 < 1 || isNaN(data0)) {
                    validate96.errors = [{ instancePath: instancePath + "/limit", schemaPath: "#/properties/limit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
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
                validate96.errors = [{ instancePath: instancePath + "/offset", schemaPath: "#/properties/offset/type", keyword: "type", params: { type: schema108.properties.offset.type }, message: "must be integer,null" }];
                return false;
              }
              if (errors === _errs4) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 4294967295 || isNaN(data1)) {
                    validate96.errors = [{ instancePath: instancePath + "/offset", schemaPath: "#/properties/offset/maximum", keyword: "maximum", params: { comparison: "<=", limit: 4294967295 }, message: "must be <= 4294967295" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate96.errors = [{ instancePath: instancePath + "/offset", schemaPath: "#/properties/offset/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
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
                      validate96.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/pumasUtf8Max", keyword: "pumasUtf8Max", params: {}, message: 'must pass "pumasUtf8Max" keyword validation' }];
                      return false;
                    }
                  } else {
                    validate96.errors = [{ instancePath: instancePath + "/query", schemaPath: "#/properties/query/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
      validate96.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate96.errors = vErrors;
  return errors === 0;
}
var validateSelectedVersionOutcome = validate97;
function validate97(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.version === void 0 && (missing0 = "version")) {
        validate97.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "success" || key0 === "version")) {
            validate97.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.success !== void 0) {
            let data0 = data.success;
            const _errs2 = errors;
            if (typeof data0 !== "boolean") {
              validate97.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
            }
            if (true !== data0) {
              validate97.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.version !== void 0) {
              const _errs4 = errors;
              if (typeof data.version !== "string") {
                validate97.errors = [{ instancePath: instancePath + "/version", schemaPath: "#/properties/version/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
      validate97.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate97.errors = vErrors;
  return errors === 0;
}
var validateSetDefaultVersionOutcome = validate98;
function validate98(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success")) {
        validate98.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "success")) {
            validate98.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.success !== void 0) {
            if (typeof data.success !== "boolean") {
              validate98.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
            }
          }
        }
      }
    } else {
      validate98.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate98.errors = vErrors;
  return errors === 0;
}
var validateSetDefaultVersionParams = validate99;
var schema111 = { "anyOf": [{ "additionalProperties": false, "properties": { "app_id": { "type": "string" }, "tag": { "default": null, "type": ["string", "null"] } }, "required": ["app_id"], "type": "object" }, { "additionalProperties": false, "properties": { "appId": { "type": "string" }, "tag": { "default": null, "type": ["string", "null"] } }, "required": ["appId"], "type": "object" }] };
function validate99(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.app_id === void 0 && (missing0 = "app_id")) {
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
          if (!(key0 === "app_id" || key0 === "tag")) {
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
          if (data.app_id !== void 0) {
            const _errs4 = errors;
            if (typeof data.app_id !== "string") {
              const err2 = { instancePath: instancePath + "/app_id", schemaPath: "#/anyOf/0/properties/app_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.tag !== void 0) {
              let data1 = data.tag;
              const _errs6 = errors;
              if (typeof data1 !== "string" && data1 !== null) {
                const err3 = { instancePath: instancePath + "/tag", schemaPath: "#/anyOf/0/properties/tag/type", keyword: "type", params: { type: schema111.anyOf[0].properties.tag.type }, message: "must be string,null" };
                if (vErrors === null) {
                  vErrors = [err3];
                } else {
                  vErrors.push(err3);
                }
                errors++;
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      const err4 = { instancePath, schemaPath: "#/anyOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs8 = errors;
    if (errors === _errs8) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.appId === void 0 && (missing1 = "appId")) {
          const err5 = { instancePath, schemaPath: "#/anyOf/1/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        } else {
          const _errs10 = errors;
          for (const key1 in data) {
            if (!(key1 === "appId" || key1 === "tag")) {
              const err6 = { instancePath, schemaPath: "#/anyOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err6];
              } else {
                vErrors.push(err6);
              }
              errors++;
              break;
            }
          }
          if (_errs10 === errors) {
            if (data.appId !== void 0) {
              const _errs11 = errors;
              if (typeof data.appId !== "string") {
                const err7 = { instancePath: instancePath + "/appId", schemaPath: "#/anyOf/1/properties/appId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err7];
                } else {
                  vErrors.push(err7);
                }
                errors++;
              }
              var valid2 = _errs11 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.tag !== void 0) {
                let data3 = data.tag;
                const _errs13 = errors;
                if (typeof data3 !== "string" && data3 !== null) {
                  const err8 = { instancePath: instancePath + "/tag", schemaPath: "#/anyOf/1/properties/tag/type", keyword: "type", params: { type: schema111.anyOf[1].properties.tag.type }, message: "must be string,null" };
                  if (vErrors === null) {
                    vErrors = [err8];
                  } else {
                    vErrors.push(err8);
                  }
                  errors++;
                }
                var valid2 = _errs13 === errors;
              } else {
                var valid2 = true;
              }
            }
          }
        }
      } else {
        const err9 = { instancePath, schemaPath: "#/anyOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    var _valid0 = _errs8 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
    validate99.errors = vErrors;
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
  validate99.errors = vErrors;
  return errors === 0;
}
var validateStartBackendSetupParams = validate100;
var schema112 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "definitions": { "QuantBackend": { "description": "Identifies which quantization backend provides a capability.", "oneOf": [{ "const": "python_conversion", "description": "Existing Python-based safetensors \u2194 GGUF F16 conversion.", "type": "string" }, { "const": "llama_cpp", "description": "llama.cpp native quantization (llama-quantize, llama-imatrix).", "type": "string" }, { "const": "nvfp4", "description": "NVIDIA NVFP4 via TensorRT-LLM / nvidia-modelopt (Phase 2).", "type": "string" }, { "const": "sherry", "description": "Sherry / AngelSlim quantization-aware training (Phase 3).", "type": "string" }] } }, "properties": { "backend": { "$ref": "#/definitions/QuantBackend" }, "expected_previous_operation_id": { "default": null, "maxLength": 36, "minLength": 36, "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "type": ["string", "null"] } }, "required": ["backend"], "title": "StartBackendSetupParams", "type": "object" };
function validate100(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.backend === void 0 && (missing0 = "backend")) {
        validate100.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backend" || key0 === "expected_previous_operation_id")) {
            validate100.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
              validate100.errors = vErrors;
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
                validate100.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/type", keyword: "type", params: { type: schema112.properties.expected_previous_operation_id.type }, message: "must be string,null" }];
                return false;
              }
              if (errors === _errs13) {
                if (typeof data1 === "string") {
                  if (func5(data1) > 36) {
                    validate100.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/maxLength", keyword: "maxLength", params: { limit: 36 }, message: "must NOT have more than 36 characters" }];
                    return false;
                  } else {
                    if (func5(data1) < 36) {
                      validate100.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/minLength", keyword: "minLength", params: { limit: 36 }, message: "must NOT have fewer than 36 characters" }];
                      return false;
                    } else {
                      if (!pattern12.test(data1)) {
                        validate100.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/pattern", keyword: "pattern", params: { pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$" }, message: 'must match pattern "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"' }];
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
      validate100.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate100.errors = vErrors;
  return errors === 0;
}
var validateStartConversionSetupParams = validate101;
var schema114 = { "$schema": "http://json-schema.org/draft-07/schema#", "additionalProperties": false, "properties": { "expected_previous_operation_id": { "default": null, "maxLength": 36, "minLength": 36, "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", "type": ["string", "null"] } }, "title": "StartConversionSetupParams", "type": "object" };
function validate101(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      const _errs1 = errors;
      for (const key0 in data) {
        if (!(key0 === "expected_previous_operation_id")) {
          validate101.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
          return false;
          break;
        }
      }
      if (_errs1 === errors) {
        if (data.expected_previous_operation_id !== void 0) {
          let data0 = data.expected_previous_operation_id;
          const _errs2 = errors;
          if (typeof data0 !== "string" && data0 !== null) {
            validate101.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/type", keyword: "type", params: { type: schema114.properties.expected_previous_operation_id.type }, message: "must be string,null" }];
            return false;
          }
          if (errors === _errs2) {
            if (typeof data0 === "string") {
              if (func5(data0) > 36) {
                validate101.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/maxLength", keyword: "maxLength", params: { limit: 36 }, message: "must NOT have more than 36 characters" }];
                return false;
              } else {
                if (func5(data0) < 36) {
                  validate101.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/minLength", keyword: "minLength", params: { limit: 36 }, message: "must NOT have fewer than 36 characters" }];
                  return false;
                } else {
                  if (!pattern12.test(data0)) {
                    validate101.errors = [{ instancePath: instancePath + "/expected_previous_operation_id", schemaPath: "#/properties/expected_previous_operation_id/pattern", keyword: "pattern", params: { pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$" }, message: 'must match pattern "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"' }];
                    return false;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate101.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate101.errors = vErrors;
  return errors === 0;
}
var validateSuccessOutcome = validate102;
function validate102(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success")) {
        validate102.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "success")) {
            validate102.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.success !== void 0) {
            let data0 = data.success;
            if (typeof data0 !== "boolean") {
              validate102.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
            }
            if (true !== data0) {
              validate102.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
              return false;
            }
          }
        }
      }
    } else {
      validate102.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate102.errors = vErrors;
  return errors === 0;
}
var validateSupportedQuantTypesOutcome = validate103;
function validate104(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.name === void 0 && (missing0 = "name") || data.description === void 0 && (missing0 = "description") || data.bitsPerWeight === void 0 && (missing0 = "bitsPerWeight") || data.recommended === void 0 && (missing0 = "recommended") || data.backend === void 0 && (missing0 = "backend") || data.imatrixRecommended === void 0 && (missing0 = "imatrixRecommended")) {
        validate104.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "backend" || key0 === "bitsPerWeight" || key0 === "description" || key0 === "imatrixRecommended" || key0 === "name" || key0 === "recommended")) {
            validate104.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
              validate104.errors = vErrors;
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
                    validate104.errors = [{ instancePath: instancePath + "/bitsPerWeight", schemaPath: "#/properties/bitsPerWeight/maximum", keyword: "maximum", params: { comparison: "<=", limit: 34028234663852886e22 }, message: "must be <= 3.4028234663852886e+38" }];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate104.errors = [{ instancePath: instancePath + "/bitsPerWeight", schemaPath: "#/properties/bitsPerWeight/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                      return false;
                    }
                  }
                } else {
                  validate104.errors = [{ instancePath: instancePath + "/bitsPerWeight", schemaPath: "#/properties/bitsPerWeight/type", keyword: "type", params: { type: "number" }, message: "must be number" }];
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
                  validate104.errors = [{ instancePath: instancePath + "/description", schemaPath: "#/properties/description/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                    validate104.errors = [{ instancePath: instancePath + "/imatrixRecommended", schemaPath: "#/properties/imatrixRecommended/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
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
                      validate104.errors = [{ instancePath: instancePath + "/name", schemaPath: "#/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                        validate104.errors = [{ instancePath: instancePath + "/recommended", schemaPath: "#/properties/recommended/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
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
      validate104.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate104.errors = vErrors;
  return errors === 0;
}
function validate103(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.quant_types === void 0 && (missing0 = "quant_types")) {
        validate103.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "quant_types" || key0 === "success")) {
            validate103.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate104(data0[i0], { instancePath: instancePath + "/quant_types/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate104.errors : vErrors.concat(validate104.errors);
                    errors = vErrors.length;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate103.errors = [{ instancePath: instancePath + "/quant_types", schemaPath: "#/properties/quant_types/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
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
                validate103.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data2) {
                validate103.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate103.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate103.errors = vErrors;
  return errors === 0;
}
var validateSwitchVersionOutcome = validate106;
function validate106(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success")) {
        validate106.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "success")) {
            validate106.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.success !== void 0) {
            if (typeof data.success !== "boolean") {
              validate106.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
              return false;
            }
          }
        }
      }
    } else {
      validate106.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate106.errors = vErrors;
  return errors === 0;
}
var validateUpdateInferenceSettingsOutcome = validate107;
function validate107(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.model_id === void 0 && (missing0 = "model_id")) {
        validate107.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "model_id" || key0 === "success")) {
            validate107.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.model_id !== void 0) {
            const _errs2 = errors;
            if (typeof data.model_id !== "string") {
              validate107.errors = [{ instancePath: instancePath + "/model_id", schemaPath: "#/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                validate107.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate107.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate107.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate107.errors = vErrors;
  return errors === 0;
}
var validateUpdateInferenceSettingsParams = validate108;
var schema122 = { "additionalProperties": false, "properties": { "constraints": { "anyOf": [{ "$ref": "#/definitions/InferenceConstraintsInput" }, { "type": "null" }] }, "default": { "$ref": "#/definitions/DesktopJsonValue" }, "description": { "default": null, "type": ["string", "null"] }, "key": { "type": "string" }, "label": { "type": "string" }, "param_type": { "$ref": "#/definitions/ParamType" } }, "required": ["key", "label", "param_type", "default"], "type": "object" };
var schema125 = { "description": "Data type for an inference parameter.", "enum": ["Number", "Integer", "String", "Boolean"], "type": "string" };
var schema123 = { "additionalProperties": false, "properties": { "allowed_values": { "anyOf": [{ "type": "null" }, { "items": { "$ref": "#/definitions/DesktopJsonValue" }, "type": "array" }] }, "max": { "default": null, "maximum": 17976931348623157e292, "minimum": -17976931348623157e292, "type": ["number", "null"] }, "min": { "default": null, "maximum": 17976931348623157e292, "minimum": -17976931348623157e292, "type": ["number", "null"] } }, "type": "object" };
var wrapper4 = { validate: validate111 };
function validate111(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
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
                if (!wrapper4.validate(data[i0], { instancePath: instancePath + "/" + i0, parentData: data, parentDataProperty: i0, rootData })) {
                  vErrors = vErrors === null ? wrapper4.validate.errors : vErrors.concat(wrapper4.validate.errors);
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
                  if (!wrapper4.validate(data[key0], { instancePath: instancePath + "/" + key0.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data, parentDataProperty: key0, rootData })) {
                    vErrors = vErrors === null ? wrapper4.validate.errors : vErrors.concat(wrapper4.validate.errors);
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
    validate111.errors = vErrors;
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
  validate111.errors = vErrors;
  return errors === 0;
}
function validate110(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      const _errs1 = errors;
      for (const key0 in data) {
        if (!(key0 === "allowed_values" || key0 === "max" || key0 === "min")) {
          validate110.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
                  if (!validate111(data0[i0], { instancePath: instancePath + "/allowed_values/" + i0, parentData: data0, parentDataProperty: i0, rootData })) {
                    vErrors = vErrors === null ? validate111.errors : vErrors.concat(validate111.errors);
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
            validate110.errors = vErrors;
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
              validate110.errors = [{ instancePath: instancePath + "/max", schemaPath: "#/properties/max/type", keyword: "type", params: { type: schema123.properties.max.type }, message: "must be number,null" }];
              return false;
            }
            if (errors === _errs9) {
              if (typeof data2 == "number" && isFinite(data2)) {
                if (data2 > 17976931348623157e292 || isNaN(data2)) {
                  validate110.errors = [{ instancePath: instancePath + "/max", schemaPath: "#/properties/max/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                  return false;
                } else {
                  if (data2 < -17976931348623157e292 || isNaN(data2)) {
                    validate110.errors = [{ instancePath: instancePath + "/max", schemaPath: "#/properties/max/minimum", keyword: "minimum", params: { comparison: ">=", limit: -17976931348623157e292 }, message: "must be >= -1.7976931348623157e+308" }];
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
                validate110.errors = [{ instancePath: instancePath + "/min", schemaPath: "#/properties/min/type", keyword: "type", params: { type: schema123.properties.min.type }, message: "must be number,null" }];
                return false;
              }
              if (errors === _errs11) {
                if (typeof data3 == "number" && isFinite(data3)) {
                  if (data3 > 17976931348623157e292 || isNaN(data3)) {
                    validate110.errors = [{ instancePath: instancePath + "/min", schemaPath: "#/properties/min/maximum", keyword: "maximum", params: { comparison: "<=", limit: 17976931348623157e292 }, message: "must be <= 1.7976931348623157e+308" }];
                    return false;
                  } else {
                    if (data3 < -17976931348623157e292 || isNaN(data3)) {
                      validate110.errors = [{ instancePath: instancePath + "/min", schemaPath: "#/properties/min/minimum", keyword: "minimum", params: { comparison: ">=", limit: -17976931348623157e292 }, message: "must be >= -1.7976931348623157e+308" }];
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
    } else {
      validate110.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate110.errors = vErrors;
  return errors === 0;
}
function validate109(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.key === void 0 && (missing0 = "key") || data.label === void 0 && (missing0 = "label") || data.param_type === void 0 && (missing0 = "param_type") || data.default === void 0 && (missing0 = "default")) {
        validate109.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "constraints" || key0 === "default" || key0 === "description" || key0 === "key" || key0 === "label" || key0 === "param_type")) {
            validate109.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
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
            if (!validate110(data0, { instancePath: instancePath + "/constraints", parentData: data, parentDataProperty: "constraints", rootData })) {
              vErrors = vErrors === null ? validate110.errors : vErrors.concat(validate110.errors);
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
              validate109.errors = vErrors;
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
              if (!validate111(data.default, { instancePath: instancePath + "/default", parentData: data, parentDataProperty: "default", rootData })) {
                vErrors = vErrors === null ? validate111.errors : vErrors.concat(validate111.errors);
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
                  validate109.errors = [{ instancePath: instancePath + "/description", schemaPath: "#/properties/description/type", keyword: "type", params: { type: schema122.properties.description.type }, message: "must be string,null" }];
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
                    validate109.errors = [{ instancePath: instancePath + "/key", schemaPath: "#/properties/key/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                      validate109.errors = [{ instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
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
                        validate109.errors = [{ instancePath: instancePath + "/param_type", schemaPath: "#/definitions/ParamType/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                        return false;
                      }
                      if (!(data5 === "Number" || data5 === "Integer" || data5 === "String" || data5 === "Boolean")) {
                        validate109.errors = [{ instancePath: instancePath + "/param_type", schemaPath: "#/definitions/ParamType/enum", keyword: "enum", params: { allowedValues: schema125.enum }, message: "must be equal to one of the allowed values" }];
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
      validate109.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate109.errors = vErrors;
  return errors === 0;
}
function validate108(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.model_id === void 0 && (missing0 = "model_id") || data.settings === void 0 && (missing0 = "settings")) {
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
          if (!(key0 === "model_id" || key0 === "settings")) {
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
          if (data.model_id !== void 0) {
            const _errs4 = errors;
            if (typeof data.model_id !== "string") {
              const err2 = { instancePath: instancePath + "/model_id", schemaPath: "#/anyOf/0/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.settings !== void 0) {
              let data1 = data.settings;
              const _errs6 = errors;
              if (errors === _errs6) {
                if (Array.isArray(data1)) {
                  var valid2 = true;
                  const len0 = data1.length;
                  for (let i0 = 0; i0 < len0; i0++) {
                    const _errs8 = errors;
                    if (!validate109(data1[i0], { instancePath: instancePath + "/settings/" + i0, parentData: data1, parentDataProperty: i0, rootData })) {
                      vErrors = vErrors === null ? validate109.errors : vErrors.concat(validate109.errors);
                      errors = vErrors.length;
                    }
                    var valid2 = _errs8 === errors;
                    if (!valid2) {
                      break;
                    }
                  }
                } else {
                  const err3 = { instancePath: instancePath + "/settings", schemaPath: "#/anyOf/0/properties/settings/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                  if (vErrors === null) {
                    vErrors = [err3];
                  } else {
                    vErrors.push(err3);
                  }
                  errors++;
                }
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      const err4 = { instancePath, schemaPath: "#/anyOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs9 = errors;
    if (errors === _errs9) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.model_id === void 0 && (missing1 = "model_id") || data.inference_settings === void 0 && (missing1 = "inference_settings")) {
          const err5 = { instancePath, schemaPath: "#/anyOf/1/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        } else {
          const _errs11 = errors;
          for (const key1 in data) {
            if (!(key1 === "inference_settings" || key1 === "model_id")) {
              const err6 = { instancePath, schemaPath: "#/anyOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err6];
              } else {
                vErrors.push(err6);
              }
              errors++;
              break;
            }
          }
          if (_errs11 === errors) {
            if (data.inference_settings !== void 0) {
              let data3 = data.inference_settings;
              const _errs12 = errors;
              if (errors === _errs12) {
                if (Array.isArray(data3)) {
                  var valid4 = true;
                  const len1 = data3.length;
                  for (let i1 = 0; i1 < len1; i1++) {
                    const _errs14 = errors;
                    if (!validate109(data3[i1], { instancePath: instancePath + "/inference_settings/" + i1, parentData: data3, parentDataProperty: i1, rootData })) {
                      vErrors = vErrors === null ? validate109.errors : vErrors.concat(validate109.errors);
                      errors = vErrors.length;
                    }
                    var valid4 = _errs14 === errors;
                    if (!valid4) {
                      break;
                    }
                  }
                } else {
                  const err7 = { instancePath: instancePath + "/inference_settings", schemaPath: "#/anyOf/1/properties/inference_settings/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                }
              }
              var valid3 = _errs12 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.model_id !== void 0) {
                const _errs15 = errors;
                if (typeof data.model_id !== "string") {
                  const err8 = { instancePath: instancePath + "/model_id", schemaPath: "#/anyOf/1/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err8];
                  } else {
                    vErrors.push(err8);
                  }
                  errors++;
                }
                var valid3 = _errs15 === errors;
              } else {
                var valid3 = true;
              }
            }
          }
        }
      } else {
        const err9 = { instancePath, schemaPath: "#/anyOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    var _valid0 = _errs9 === errors;
    valid0 = valid0 || _valid0;
    if (!valid0) {
      const _errs17 = errors;
      if (errors === _errs17) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing2;
          if (data.model_id === void 0 && (missing2 = "model_id") || data.inferenceSettings === void 0 && (missing2 = "inferenceSettings")) {
            const err10 = { instancePath, schemaPath: "#/anyOf/2/required", keyword: "required", params: { missingProperty: missing2 }, message: "must have required property '" + missing2 + "'" };
            if (vErrors === null) {
              vErrors = [err10];
            } else {
              vErrors.push(err10);
            }
            errors++;
          } else {
            const _errs19 = errors;
            for (const key2 in data) {
              if (!(key2 === "inferenceSettings" || key2 === "model_id")) {
                const err11 = { instancePath, schemaPath: "#/anyOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
                if (vErrors === null) {
                  vErrors = [err11];
                } else {
                  vErrors.push(err11);
                }
                errors++;
                break;
              }
            }
            if (_errs19 === errors) {
              if (data.inferenceSettings !== void 0) {
                let data6 = data.inferenceSettings;
                const _errs20 = errors;
                if (errors === _errs20) {
                  if (Array.isArray(data6)) {
                    var valid6 = true;
                    const len2 = data6.length;
                    for (let i2 = 0; i2 < len2; i2++) {
                      const _errs22 = errors;
                      if (!validate109(data6[i2], { instancePath: instancePath + "/inferenceSettings/" + i2, parentData: data6, parentDataProperty: i2, rootData })) {
                        vErrors = vErrors === null ? validate109.errors : vErrors.concat(validate109.errors);
                        errors = vErrors.length;
                      }
                      var valid6 = _errs22 === errors;
                      if (!valid6) {
                        break;
                      }
                    }
                  } else {
                    const err12 = { instancePath: instancePath + "/inferenceSettings", schemaPath: "#/anyOf/2/properties/inferenceSettings/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                    if (vErrors === null) {
                      vErrors = [err12];
                    } else {
                      vErrors.push(err12);
                    }
                    errors++;
                  }
                }
                var valid5 = _errs20 === errors;
              } else {
                var valid5 = true;
              }
              if (valid5) {
                if (data.model_id !== void 0) {
                  const _errs23 = errors;
                  if (typeof data.model_id !== "string") {
                    const err13 = { instancePath: instancePath + "/model_id", schemaPath: "#/anyOf/2/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err13];
                    } else {
                      vErrors.push(err13);
                    }
                    errors++;
                  }
                  var valid5 = _errs23 === errors;
                } else {
                  var valid5 = true;
                }
              }
            }
          }
        } else {
          const err14 = { instancePath, schemaPath: "#/anyOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
      }
      var _valid0 = _errs17 === errors;
      valid0 = valid0 || _valid0;
      if (!valid0) {
        const _errs25 = errors;
        if (errors === _errs25) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing3;
            if (data.modelId === void 0 && (missing3 = "modelId") || data.settings === void 0 && (missing3 = "settings")) {
              const err15 = { instancePath, schemaPath: "#/anyOf/3/required", keyword: "required", params: { missingProperty: missing3 }, message: "must have required property '" + missing3 + "'" };
              if (vErrors === null) {
                vErrors = [err15];
              } else {
                vErrors.push(err15);
              }
              errors++;
            } else {
              const _errs27 = errors;
              for (const key3 in data) {
                if (!(key3 === "modelId" || key3 === "settings")) {
                  const err16 = { instancePath, schemaPath: "#/anyOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
                  if (vErrors === null) {
                    vErrors = [err16];
                  } else {
                    vErrors.push(err16);
                  }
                  errors++;
                  break;
                }
              }
              if (_errs27 === errors) {
                if (data.modelId !== void 0) {
                  const _errs28 = errors;
                  if (typeof data.modelId !== "string") {
                    const err17 = { instancePath: instancePath + "/modelId", schemaPath: "#/anyOf/3/properties/modelId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err17];
                    } else {
                      vErrors.push(err17);
                    }
                    errors++;
                  }
                  var valid7 = _errs28 === errors;
                } else {
                  var valid7 = true;
                }
                if (valid7) {
                  if (data.settings !== void 0) {
                    let data10 = data.settings;
                    const _errs30 = errors;
                    if (errors === _errs30) {
                      if (Array.isArray(data10)) {
                        var valid8 = true;
                        const len3 = data10.length;
                        for (let i3 = 0; i3 < len3; i3++) {
                          const _errs32 = errors;
                          if (!validate109(data10[i3], { instancePath: instancePath + "/settings/" + i3, parentData: data10, parentDataProperty: i3, rootData })) {
                            vErrors = vErrors === null ? validate109.errors : vErrors.concat(validate109.errors);
                            errors = vErrors.length;
                          }
                          var valid8 = _errs32 === errors;
                          if (!valid8) {
                            break;
                          }
                        }
                      } else {
                        const err18 = { instancePath: instancePath + "/settings", schemaPath: "#/anyOf/3/properties/settings/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                        if (vErrors === null) {
                          vErrors = [err18];
                        } else {
                          vErrors.push(err18);
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
          } else {
            const err19 = { instancePath, schemaPath: "#/anyOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
            if (vErrors === null) {
              vErrors = [err19];
            } else {
              vErrors.push(err19);
            }
            errors++;
          }
        }
        var _valid0 = _errs25 === errors;
        valid0 = valid0 || _valid0;
        if (!valid0) {
          const _errs33 = errors;
          if (errors === _errs33) {
            if (data && typeof data == "object" && !Array.isArray(data)) {
              let missing4;
              if (data.modelId === void 0 && (missing4 = "modelId") || data.inference_settings === void 0 && (missing4 = "inference_settings")) {
                const err20 = { instancePath, schemaPath: "#/anyOf/4/required", keyword: "required", params: { missingProperty: missing4 }, message: "must have required property '" + missing4 + "'" };
                if (vErrors === null) {
                  vErrors = [err20];
                } else {
                  vErrors.push(err20);
                }
                errors++;
              } else {
                const _errs35 = errors;
                for (const key4 in data) {
                  if (!(key4 === "inference_settings" || key4 === "modelId")) {
                    const err21 = { instancePath, schemaPath: "#/anyOf/4/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key4 }, message: "must NOT have additional properties" };
                    if (vErrors === null) {
                      vErrors = [err21];
                    } else {
                      vErrors.push(err21);
                    }
                    errors++;
                    break;
                  }
                }
                if (_errs35 === errors) {
                  if (data.inference_settings !== void 0) {
                    let data12 = data.inference_settings;
                    const _errs36 = errors;
                    if (errors === _errs36) {
                      if (Array.isArray(data12)) {
                        var valid10 = true;
                        const len4 = data12.length;
                        for (let i4 = 0; i4 < len4; i4++) {
                          const _errs38 = errors;
                          if (!validate109(data12[i4], { instancePath: instancePath + "/inference_settings/" + i4, parentData: data12, parentDataProperty: i4, rootData })) {
                            vErrors = vErrors === null ? validate109.errors : vErrors.concat(validate109.errors);
                            errors = vErrors.length;
                          }
                          var valid10 = _errs38 === errors;
                          if (!valid10) {
                            break;
                          }
                        }
                      } else {
                        const err22 = { instancePath: instancePath + "/inference_settings", schemaPath: "#/anyOf/4/properties/inference_settings/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                        if (vErrors === null) {
                          vErrors = [err22];
                        } else {
                          vErrors.push(err22);
                        }
                        errors++;
                      }
                    }
                    var valid9 = _errs36 === errors;
                  } else {
                    var valid9 = true;
                  }
                  if (valid9) {
                    if (data.modelId !== void 0) {
                      const _errs39 = errors;
                      if (typeof data.modelId !== "string") {
                        const err23 = { instancePath: instancePath + "/modelId", schemaPath: "#/anyOf/4/properties/modelId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                        if (vErrors === null) {
                          vErrors = [err23];
                        } else {
                          vErrors.push(err23);
                        }
                        errors++;
                      }
                      var valid9 = _errs39 === errors;
                    } else {
                      var valid9 = true;
                    }
                  }
                }
              }
            } else {
              const err24 = { instancePath, schemaPath: "#/anyOf/4/type", keyword: "type", params: { type: "object" }, message: "must be object" };
              if (vErrors === null) {
                vErrors = [err24];
              } else {
                vErrors.push(err24);
              }
              errors++;
            }
          }
          var _valid0 = _errs33 === errors;
          valid0 = valid0 || _valid0;
          if (!valid0) {
            const _errs41 = errors;
            if (errors === _errs41) {
              if (data && typeof data == "object" && !Array.isArray(data)) {
                let missing5;
                if (data.modelId === void 0 && (missing5 = "modelId") || data.inferenceSettings === void 0 && (missing5 = "inferenceSettings")) {
                  const err25 = { instancePath, schemaPath: "#/anyOf/5/required", keyword: "required", params: { missingProperty: missing5 }, message: "must have required property '" + missing5 + "'" };
                  if (vErrors === null) {
                    vErrors = [err25];
                  } else {
                    vErrors.push(err25);
                  }
                  errors++;
                } else {
                  const _errs43 = errors;
                  for (const key5 in data) {
                    if (!(key5 === "inferenceSettings" || key5 === "modelId")) {
                      const err26 = { instancePath, schemaPath: "#/anyOf/5/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key5 }, message: "must NOT have additional properties" };
                      if (vErrors === null) {
                        vErrors = [err26];
                      } else {
                        vErrors.push(err26);
                      }
                      errors++;
                      break;
                    }
                  }
                  if (_errs43 === errors) {
                    if (data.inferenceSettings !== void 0) {
                      let data15 = data.inferenceSettings;
                      const _errs44 = errors;
                      if (errors === _errs44) {
                        if (Array.isArray(data15)) {
                          var valid12 = true;
                          const len5 = data15.length;
                          for (let i5 = 0; i5 < len5; i5++) {
                            const _errs46 = errors;
                            if (!validate109(data15[i5], { instancePath: instancePath + "/inferenceSettings/" + i5, parentData: data15, parentDataProperty: i5, rootData })) {
                              vErrors = vErrors === null ? validate109.errors : vErrors.concat(validate109.errors);
                              errors = vErrors.length;
                            }
                            var valid12 = _errs46 === errors;
                            if (!valid12) {
                              break;
                            }
                          }
                        } else {
                          const err27 = { instancePath: instancePath + "/inferenceSettings", schemaPath: "#/anyOf/5/properties/inferenceSettings/type", keyword: "type", params: { type: "array" }, message: "must be array" };
                          if (vErrors === null) {
                            vErrors = [err27];
                          } else {
                            vErrors.push(err27);
                          }
                          errors++;
                        }
                      }
                      var valid11 = _errs44 === errors;
                    } else {
                      var valid11 = true;
                    }
                    if (valid11) {
                      if (data.modelId !== void 0) {
                        const _errs47 = errors;
                        if (typeof data.modelId !== "string") {
                          const err28 = { instancePath: instancePath + "/modelId", schemaPath: "#/anyOf/5/properties/modelId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                          if (vErrors === null) {
                            vErrors = [err28];
                          } else {
                            vErrors.push(err28);
                          }
                          errors++;
                        }
                        var valid11 = _errs47 === errors;
                      } else {
                        var valid11 = true;
                      }
                    }
                  }
                }
              } else {
                const err29 = { instancePath, schemaPath: "#/anyOf/5/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                if (vErrors === null) {
                  vErrors = [err29];
                } else {
                  vErrors.push(err29);
                }
                errors++;
              }
            }
            var _valid0 = _errs41 === errors;
            valid0 = valid0 || _valid0;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err30 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err30];
    } else {
      vErrors.push(err30);
    }
    errors++;
    validate108.errors = vErrors;
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
  validate108.errors = vErrors;
  return errors === 0;
}
var validateUpdateModelNotesOutcome = validate121;
function validate121(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  const _errs2 = errors;
  if (errors === _errs2) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.model_id === void 0 && (missing0 = "model_id")) {
        const err0 = { instancePath, schemaPath: "#/definitions/UpdateModelNotesSuccess/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs4 = errors;
        for (const key0 in data) {
          if (!(key0 === "model_id" || key0 === "notes" || key0 === "success")) {
            const err1 = { instancePath, schemaPath: "#/definitions/UpdateModelNotesSuccess/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
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
          if (data.model_id !== void 0) {
            const _errs5 = errors;
            if (typeof data.model_id !== "string") {
              const err2 = { instancePath: instancePath + "/model_id", schemaPath: "#/definitions/UpdateModelNotesSuccess/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
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
            if (data.notes !== void 0) {
              const _errs7 = errors;
              if (typeof data.notes !== "string") {
                const err3 = { instancePath: instancePath + "/notes", schemaPath: "#/definitions/UpdateModelNotesSuccess/properties/notes/type", keyword: "type", params: { type: "string" }, message: "must be string" };
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
              if (data.success !== void 0) {
                let data2 = data.success;
                const _errs9 = errors;
                if (typeof data2 !== "boolean") {
                  const err4 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/UpdateModelNotesSuccess/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                  if (vErrors === null) {
                    vErrors = [err4];
                  } else {
                    vErrors.push(err4);
                  }
                  errors++;
                }
                if (true !== data2) {
                  const err5 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/UpdateModelNotesSuccess/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err5];
                  } else {
                    vErrors.push(err5);
                  }
                  errors++;
                }
                var valid2 = _errs9 === errors;
              } else {
                var valid2 = true;
              }
            }
          }
        }
      }
    } else {
      const err6 = { instancePath, schemaPath: "#/definitions/UpdateModelNotesSuccess/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs11 = errors;
    const _errs12 = errors;
    if (errors === _errs12) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.success === void 0 && (missing1 = "success") || data.model_id === void 0 && (missing1 = "model_id") || data.error === void 0 && (missing1 = "error")) {
          const err7 = { instancePath, schemaPath: "#/definitions/UpdateModelNotesFailure/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err7];
          } else {
            vErrors.push(err7);
          }
          errors++;
        } else {
          const _errs14 = errors;
          for (const key1 in data) {
            if (!(key1 === "error" || key1 === "model_id" || key1 === "success")) {
              const err8 = { instancePath, schemaPath: "#/definitions/UpdateModelNotesFailure/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err8];
              } else {
                vErrors.push(err8);
              }
              errors++;
              break;
            }
          }
          if (_errs14 === errors) {
            if (data.error !== void 0) {
              const _errs15 = errors;
              if (typeof data.error !== "string") {
                const err9 = { instancePath: instancePath + "/error", schemaPath: "#/definitions/UpdateModelNotesFailure/properties/error/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err9];
                } else {
                  vErrors.push(err9);
                }
                errors++;
              }
              var valid4 = _errs15 === errors;
            } else {
              var valid4 = true;
            }
            if (valid4) {
              if (data.model_id !== void 0) {
                const _errs17 = errors;
                if (typeof data.model_id !== "string") {
                  const err10 = { instancePath: instancePath + "/model_id", schemaPath: "#/definitions/UpdateModelNotesFailure/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err10];
                  } else {
                    vErrors.push(err10);
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
                    const err11 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/UpdateModelNotesFailure/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                    if (vErrors === null) {
                      vErrors = [err11];
                    } else {
                      vErrors.push(err11);
                    }
                    errors++;
                  }
                  if (false !== data5) {
                    const err12 = { instancePath: instancePath + "/success", schemaPath: "#/definitions/UpdateModelNotesFailure/properties/success/const", keyword: "const", params: { allowedValue: false }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err12];
                    } else {
                      vErrors.push(err12);
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
        }
      } else {
        const err13 = { instancePath, schemaPath: "#/definitions/UpdateModelNotesFailure/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      }
    }
    var _valid0 = _errs11 === errors;
    valid0 = valid0 || _valid0;
  }
  if (!valid0) {
    const err14 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err14];
    } else {
      vErrors.push(err14);
    }
    errors++;
    validate121.errors = vErrors;
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
  validate121.errors = vErrors;
  return errors === 0;
}
var validateUpdateModelNotesParams = validate122;
var schema129 = { "anyOf": [{ "additionalProperties": false, "properties": { "model_id": { "type": "string" }, "notes": { "default": null, "type": ["string", "null"] } }, "required": ["model_id"], "type": "object" }, { "additionalProperties": false, "properties": { "model_id": { "type": "string" }, "model_notes": { "default": null, "type": ["string", "null"] } }, "required": ["model_id"], "type": "object" }, { "additionalProperties": false, "properties": { "modelId": { "type": "string" }, "notes": { "default": null, "type": ["string", "null"] } }, "required": ["modelId"], "type": "object" }, { "additionalProperties": false, "properties": { "modelId": { "type": "string" }, "model_notes": { "default": null, "type": ["string", "null"] } }, "required": ["modelId"], "type": "object" }] };
function validate122(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  const _errs0 = errors;
  let valid0 = false;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.model_id === void 0 && (missing0 = "model_id")) {
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
          if (!(key0 === "model_id" || key0 === "notes")) {
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
          if (data.model_id !== void 0) {
            const _errs4 = errors;
            if (typeof data.model_id !== "string") {
              const err2 = { instancePath: instancePath + "/model_id", schemaPath: "#/anyOf/0/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.notes !== void 0) {
              let data1 = data.notes;
              const _errs6 = errors;
              if (typeof data1 !== "string" && data1 !== null) {
                const err3 = { instancePath: instancePath + "/notes", schemaPath: "#/anyOf/0/properties/notes/type", keyword: "type", params: { type: schema129.anyOf[0].properties.notes.type }, message: "must be string,null" };
                if (vErrors === null) {
                  vErrors = [err3];
                } else {
                  vErrors.push(err3);
                }
                errors++;
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      const err4 = { instancePath, schemaPath: "#/anyOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  valid0 = valid0 || _valid0;
  if (!valid0) {
    const _errs8 = errors;
    if (errors === _errs8) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing1;
        if (data.model_id === void 0 && (missing1 = "model_id")) {
          const err5 = { instancePath, schemaPath: "#/anyOf/1/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        } else {
          const _errs10 = errors;
          for (const key1 in data) {
            if (!(key1 === "model_id" || key1 === "model_notes")) {
              const err6 = { instancePath, schemaPath: "#/anyOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err6];
              } else {
                vErrors.push(err6);
              }
              errors++;
              break;
            }
          }
          if (_errs10 === errors) {
            if (data.model_id !== void 0) {
              const _errs11 = errors;
              if (typeof data.model_id !== "string") {
                const err7 = { instancePath: instancePath + "/model_id", schemaPath: "#/anyOf/1/properties/model_id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err7];
                } else {
                  vErrors.push(err7);
                }
                errors++;
              }
              var valid2 = _errs11 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.model_notes !== void 0) {
                let data3 = data.model_notes;
                const _errs13 = errors;
                if (typeof data3 !== "string" && data3 !== null) {
                  const err8 = { instancePath: instancePath + "/model_notes", schemaPath: "#/anyOf/1/properties/model_notes/type", keyword: "type", params: { type: schema129.anyOf[1].properties.model_notes.type }, message: "must be string,null" };
                  if (vErrors === null) {
                    vErrors = [err8];
                  } else {
                    vErrors.push(err8);
                  }
                  errors++;
                }
                var valid2 = _errs13 === errors;
              } else {
                var valid2 = true;
              }
            }
          }
        }
      } else {
        const err9 = { instancePath, schemaPath: "#/anyOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    var _valid0 = _errs8 === errors;
    valid0 = valid0 || _valid0;
    if (!valid0) {
      const _errs15 = errors;
      if (errors === _errs15) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing2;
          if (data.modelId === void 0 && (missing2 = "modelId")) {
            const err10 = { instancePath, schemaPath: "#/anyOf/2/required", keyword: "required", params: { missingProperty: missing2 }, message: "must have required property '" + missing2 + "'" };
            if (vErrors === null) {
              vErrors = [err10];
            } else {
              vErrors.push(err10);
            }
            errors++;
          } else {
            const _errs17 = errors;
            for (const key2 in data) {
              if (!(key2 === "modelId" || key2 === "notes")) {
                const err11 = { instancePath, schemaPath: "#/anyOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
                if (vErrors === null) {
                  vErrors = [err11];
                } else {
                  vErrors.push(err11);
                }
                errors++;
                break;
              }
            }
            if (_errs17 === errors) {
              if (data.modelId !== void 0) {
                const _errs18 = errors;
                if (typeof data.modelId !== "string") {
                  const err12 = { instancePath: instancePath + "/modelId", schemaPath: "#/anyOf/2/properties/modelId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err12];
                  } else {
                    vErrors.push(err12);
                  }
                  errors++;
                }
                var valid3 = _errs18 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.notes !== void 0) {
                  let data5 = data.notes;
                  const _errs20 = errors;
                  if (typeof data5 !== "string" && data5 !== null) {
                    const err13 = { instancePath: instancePath + "/notes", schemaPath: "#/anyOf/2/properties/notes/type", keyword: "type", params: { type: schema129.anyOf[2].properties.notes.type }, message: "must be string,null" };
                    if (vErrors === null) {
                      vErrors = [err13];
                    } else {
                      vErrors.push(err13);
                    }
                    errors++;
                  }
                  var valid3 = _errs20 === errors;
                } else {
                  var valid3 = true;
                }
              }
            }
          }
        } else {
          const err14 = { instancePath, schemaPath: "#/anyOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
      }
      var _valid0 = _errs15 === errors;
      valid0 = valid0 || _valid0;
      if (!valid0) {
        const _errs22 = errors;
        if (errors === _errs22) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing3;
            if (data.modelId === void 0 && (missing3 = "modelId")) {
              const err15 = { instancePath, schemaPath: "#/anyOf/3/required", keyword: "required", params: { missingProperty: missing3 }, message: "must have required property '" + missing3 + "'" };
              if (vErrors === null) {
                vErrors = [err15];
              } else {
                vErrors.push(err15);
              }
              errors++;
            } else {
              const _errs24 = errors;
              for (const key3 in data) {
                if (!(key3 === "modelId" || key3 === "model_notes")) {
                  const err16 = { instancePath, schemaPath: "#/anyOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
                  if (vErrors === null) {
                    vErrors = [err16];
                  } else {
                    vErrors.push(err16);
                  }
                  errors++;
                  break;
                }
              }
              if (_errs24 === errors) {
                if (data.modelId !== void 0) {
                  const _errs25 = errors;
                  if (typeof data.modelId !== "string") {
                    const err17 = { instancePath: instancePath + "/modelId", schemaPath: "#/anyOf/3/properties/modelId/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err17];
                    } else {
                      vErrors.push(err17);
                    }
                    errors++;
                  }
                  var valid4 = _errs25 === errors;
                } else {
                  var valid4 = true;
                }
                if (valid4) {
                  if (data.model_notes !== void 0) {
                    let data7 = data.model_notes;
                    const _errs27 = errors;
                    if (typeof data7 !== "string" && data7 !== null) {
                      const err18 = { instancePath: instancePath + "/model_notes", schemaPath: "#/anyOf/3/properties/model_notes/type", keyword: "type", params: { type: schema129.anyOf[3].properties.model_notes.type }, message: "must be string,null" };
                      if (vErrors === null) {
                        vErrors = [err18];
                      } else {
                        vErrors.push(err18);
                      }
                      errors++;
                    }
                    var valid4 = _errs27 === errors;
                  } else {
                    var valid4 = true;
                  }
                }
              }
            }
          } else {
            const err19 = { instancePath, schemaPath: "#/anyOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
            if (vErrors === null) {
              vErrors = [err19];
            } else {
              vErrors.push(err19);
            }
            errors++;
          }
        }
        var _valid0 = _errs22 === errors;
        valid0 = valid0 || _valid0;
      }
    }
  }
  if (!valid0) {
    const err20 = { instancePath, schemaPath: "#/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
    if (vErrors === null) {
      vErrors = [err20];
    } else {
      vErrors.push(err20);
    }
    errors++;
    validate122.errors = vErrors;
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
  validate122.errors = vErrors;
  return errors === 0;
}
var validateValidateInstallationsOutcome = validate123;
function validate123(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.removed_tags === void 0 && (missing0 = "removed_tags") || data.orphaned_dirs === void 0 && (missing0 = "orphaned_dirs") || data.valid_count === void 0 && (missing0 = "valid_count")) {
        validate123.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "orphaned_dirs" || key0 === "removed_tags" || key0 === "valid_count")) {
            validate123.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.orphaned_dirs !== void 0) {
            let data0 = data.orphaned_dirs;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (Array.isArray(data0)) {
                var valid1 = true;
                const len0 = data0.length;
                for (let i0 = 0; i0 < len0; i0++) {
                  const _errs4 = errors;
                  if (typeof data0[i0] !== "string") {
                    validate123.errors = [{ instancePath: instancePath + "/orphaned_dirs/" + i0, schemaPath: "#/properties/orphaned_dirs/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                    return false;
                  }
                  var valid1 = _errs4 === errors;
                  if (!valid1) {
                    break;
                  }
                }
              } else {
                validate123.errors = [{ instancePath: instancePath + "/orphaned_dirs", schemaPath: "#/properties/orphaned_dirs/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.removed_tags !== void 0) {
              let data2 = data.removed_tags;
              const _errs6 = errors;
              if (errors === _errs6) {
                if (Array.isArray(data2)) {
                  var valid2 = true;
                  const len1 = data2.length;
                  for (let i1 = 0; i1 < len1; i1++) {
                    const _errs8 = errors;
                    if (typeof data2[i1] !== "string") {
                      validate123.errors = [{ instancePath: instancePath + "/removed_tags/" + i1, schemaPath: "#/properties/removed_tags/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                      return false;
                    }
                    var valid2 = _errs8 === errors;
                    if (!valid2) {
                      break;
                    }
                  }
                } else {
                  validate123.errors = [{ instancePath: instancePath + "/removed_tags", schemaPath: "#/properties/removed_tags/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                  return false;
                }
              }
              var valid0 = _errs6 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.valid_count !== void 0) {
                let data4 = data.valid_count;
                const _errs10 = errors;
                if (!(typeof data4 == "number" && (!(data4 % 1) && !isNaN(data4)) && isFinite(data4))) {
                  validate123.errors = [{ instancePath: instancePath + "/valid_count", schemaPath: "#/properties/valid_count/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                  return false;
                }
                if (errors === _errs10) {
                  if (typeof data4 == "number" && isFinite(data4)) {
                    if (data4 > 9007199254740991 || isNaN(data4)) {
                      validate123.errors = [{ instancePath: instancePath + "/valid_count", schemaPath: "#/properties/valid_count/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                      return false;
                    } else {
                      if (data4 < 0 || isNaN(data4)) {
                        validate123.errors = [{ instancePath: instancePath + "/valid_count", schemaPath: "#/properties/valid_count/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                        return false;
                      }
                    }
                  }
                }
                var valid0 = _errs10 === errors;
              } else {
                var valid0 = true;
              }
            }
          }
        }
      }
    } else {
      validate123.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate123.errors = vErrors;
  return errors === 0;
}
var validateVersionInfoOutcome = validate124;
function validate124(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.info === void 0 && (missing0 = "info")) {
        validate124.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "info" || key0 === "success")) {
            validate124.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.info !== void 0) {
            let data0 = data.info;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (data0 && typeof data0 == "object" && !Array.isArray(data0)) {
                let missing1;
                if (data0.tag === void 0 && (missing1 = "tag") || data0.installed === void 0 && (missing1 = "installed") || data0.size === void 0 && (missing1 = "size")) {
                  validate124.errors = [{ instancePath: instancePath + "/info", schemaPath: "#/definitions/RuntimeVersionInfo/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" }];
                  return false;
                } else {
                  const _errs5 = errors;
                  for (const key1 in data0) {
                    if (!(key1 === "installed" || key1 === "size" || key1 === "tag")) {
                      validate124.errors = [{ instancePath: instancePath + "/info", schemaPath: "#/definitions/RuntimeVersionInfo/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" }];
                      return false;
                      break;
                    }
                  }
                  if (_errs5 === errors) {
                    if (data0.installed !== void 0) {
                      const _errs6 = errors;
                      if (typeof data0.installed !== "boolean") {
                        validate124.errors = [{ instancePath: instancePath + "/info/installed", schemaPath: "#/definitions/RuntimeVersionInfo/properties/installed/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                        return false;
                      }
                      var valid2 = _errs6 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data0.size !== void 0) {
                        const _errs8 = errors;
                        if (data0.size !== null) {
                          validate124.errors = [{ instancePath: instancePath + "/info/size", schemaPath: "#/definitions/RuntimeVersionInfo/properties/size/type", keyword: "type", params: { type: "null" }, message: "must be null" }];
                          return false;
                        }
                        var valid2 = _errs8 === errors;
                      } else {
                        var valid2 = true;
                      }
                      if (valid2) {
                        if (data0.tag !== void 0) {
                          const _errs10 = errors;
                          if (typeof data0.tag !== "string") {
                            validate124.errors = [{ instancePath: instancePath + "/info/tag", schemaPath: "#/definitions/RuntimeVersionInfo/properties/tag/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                            return false;
                          }
                          var valid2 = _errs10 === errors;
                        } else {
                          var valid2 = true;
                        }
                      }
                    }
                  }
                }
              } else {
                validate124.errors = [{ instancePath: instancePath + "/info", schemaPath: "#/definitions/RuntimeVersionInfo/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.success !== void 0) {
              let data4 = data.success;
              const _errs12 = errors;
              if (typeof data4 !== "boolean") {
                validate124.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data4) {
                validate124.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                return false;
              }
              var valid0 = _errs12 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate124.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate124.errors = vErrors;
  return errors === 0;
}
var validateVersionStatusOutcome = validate125;
var schema134 = { "additionalProperties": false, "properties": { "activeVersion": { "type": ["string", "null"] }, "defaultVersion": { "type": ["string", "null"] }, "installedCount": { "maximum": 9007199254740991, "minimum": 0, "type": "integer" }, "versions": { "additionalProperties": { "$ref": "#/definitions/RuntimeVersionEntry" }, "type": "object" } }, "required": ["installedCount", "activeVersion", "defaultVersion", "versions"], "type": "object" };
function validate127(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.isActive === void 0 && (missing0 = "isActive") || data.dependencies === void 0 && (missing0 = "dependencies")) {
        validate127.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "dependencies" || key0 === "isActive")) {
            validate127.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.dependencies !== void 0) {
            let data0 = data.dependencies;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (data0 && typeof data0 == "object" && !Array.isArray(data0)) {
                let missing1;
                if (data0.installed === void 0 && (missing1 = "installed") || data0.missing === void 0 && (missing1 = "missing")) {
                  validate127.errors = [{ instancePath: instancePath + "/dependencies", schemaPath: "#/definitions/RuntimeVersionDependencies/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" }];
                  return false;
                } else {
                  const _errs5 = errors;
                  for (const key1 in data0) {
                    if (!(key1 === "installed" || key1 === "missing")) {
                      validate127.errors = [{ instancePath: instancePath + "/dependencies", schemaPath: "#/definitions/RuntimeVersionDependencies/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" }];
                      return false;
                      break;
                    }
                  }
                  if (_errs5 === errors) {
                    if (data0.installed !== void 0) {
                      let data1 = data0.installed;
                      const _errs6 = errors;
                      if (errors === _errs6) {
                        if (Array.isArray(data1)) {
                          var valid3 = true;
                          const len0 = data1.length;
                          for (let i0 = 0; i0 < len0; i0++) {
                            const _errs8 = errors;
                            if (typeof data1[i0] !== "string") {
                              validate127.errors = [{ instancePath: instancePath + "/dependencies/installed/" + i0, schemaPath: "#/definitions/RuntimeVersionDependencies/properties/installed/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                              return false;
                            }
                            var valid3 = _errs8 === errors;
                            if (!valid3) {
                              break;
                            }
                          }
                        } else {
                          validate127.errors = [{ instancePath: instancePath + "/dependencies/installed", schemaPath: "#/definitions/RuntimeVersionDependencies/properties/installed/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                          return false;
                        }
                      }
                      var valid2 = _errs6 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data0.missing !== void 0) {
                        let data3 = data0.missing;
                        const _errs10 = errors;
                        if (errors === _errs10) {
                          if (Array.isArray(data3)) {
                            var valid4 = true;
                            const len1 = data3.length;
                            for (let i1 = 0; i1 < len1; i1++) {
                              const _errs12 = errors;
                              if (typeof data3[i1] !== "string") {
                                validate127.errors = [{ instancePath: instancePath + "/dependencies/missing/" + i1, schemaPath: "#/definitions/RuntimeVersionDependencies/properties/missing/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                return false;
                              }
                              var valid4 = _errs12 === errors;
                              if (!valid4) {
                                break;
                              }
                            }
                          } else {
                            validate127.errors = [{ instancePath: instancePath + "/dependencies/missing", schemaPath: "#/definitions/RuntimeVersionDependencies/properties/missing/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                            return false;
                          }
                        }
                        var valid2 = _errs10 === errors;
                      } else {
                        var valid2 = true;
                      }
                    }
                  }
                }
              } else {
                validate127.errors = [{ instancePath: instancePath + "/dependencies", schemaPath: "#/definitions/RuntimeVersionDependencies/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.isActive !== void 0) {
              const _errs14 = errors;
              if (typeof data.isActive !== "boolean") {
                validate127.errors = [{ instancePath: instancePath + "/isActive", schemaPath: "#/properties/isActive/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              var valid0 = _errs14 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate127.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate127.errors = vErrors;
  return errors === 0;
}
function validate126(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.installedCount === void 0 && (missing0 = "installedCount") || data.activeVersion === void 0 && (missing0 = "activeVersion") || data.defaultVersion === void 0 && (missing0 = "defaultVersion") || data.versions === void 0 && (missing0 = "versions")) {
        validate126.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "activeVersion" || key0 === "defaultVersion" || key0 === "installedCount" || key0 === "versions")) {
            validate126.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.activeVersion !== void 0) {
            let data0 = data.activeVersion;
            const _errs2 = errors;
            if (typeof data0 !== "string" && data0 !== null) {
              validate126.errors = [{ instancePath: instancePath + "/activeVersion", schemaPath: "#/properties/activeVersion/type", keyword: "type", params: { type: schema134.properties.activeVersion.type }, message: "must be string,null" }];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.defaultVersion !== void 0) {
              let data1 = data.defaultVersion;
              const _errs4 = errors;
              if (typeof data1 !== "string" && data1 !== null) {
                validate126.errors = [{ instancePath: instancePath + "/defaultVersion", schemaPath: "#/properties/defaultVersion/type", keyword: "type", params: { type: schema134.properties.defaultVersion.type }, message: "must be string,null" }];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.installedCount !== void 0) {
                let data2 = data.installedCount;
                const _errs6 = errors;
                if (!(typeof data2 == "number" && (!(data2 % 1) && !isNaN(data2)) && isFinite(data2))) {
                  validate126.errors = [{ instancePath: instancePath + "/installedCount", schemaPath: "#/properties/installedCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                  return false;
                }
                if (errors === _errs6) {
                  if (typeof data2 == "number" && isFinite(data2)) {
                    if (data2 > 9007199254740991 || isNaN(data2)) {
                      validate126.errors = [{ instancePath: instancePath + "/installedCount", schemaPath: "#/properties/installedCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 9007199254740991 }, message: "must be <= 9007199254740991" }];
                      return false;
                    } else {
                      if (data2 < 0 || isNaN(data2)) {
                        validate126.errors = [{ instancePath: instancePath + "/installedCount", schemaPath: "#/properties/installedCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                        return false;
                      }
                    }
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.versions !== void 0) {
                  let data3 = data.versions;
                  const _errs8 = errors;
                  if (errors === _errs8) {
                    if (data3 && typeof data3 == "object" && !Array.isArray(data3)) {
                      for (const key1 in data3) {
                        const _errs11 = errors;
                        if (!validate127(data3[key1], { instancePath: instancePath + "/versions/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"), parentData: data3, parentDataProperty: key1, rootData })) {
                          vErrors = vErrors === null ? validate127.errors : vErrors.concat(validate127.errors);
                          errors = vErrors.length;
                        }
                        var valid1 = _errs11 === errors;
                        if (!valid1) {
                          break;
                        }
                      }
                    } else {
                      validate126.errors = [{ instancePath: instancePath + "/versions", schemaPath: "#/properties/versions/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                      return false;
                    }
                  }
                  var valid0 = _errs8 === errors;
                } else {
                  var valid0 = true;
                }
              }
            }
          }
        }
      }
    } else {
      validate126.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate126.errors = vErrors;
  return errors === 0;
}
function validate125(data, { instancePath = "", parentData, parentDataProperty, rootData = data } = {}) {
  let vErrors = null;
  let errors = 0;
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.success === void 0 && (missing0 = "success") || data.status === void 0 && (missing0 = "status")) {
        validate125.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "status" || key0 === "success")) {
            validate125.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.status !== void 0) {
            const _errs2 = errors;
            if (!validate126(data.status, { instancePath: instancePath + "/status", parentData: data, parentDataProperty: "status", rootData })) {
              vErrors = vErrors === null ? validate126.errors : vErrors.concat(validate126.errors);
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
                validate125.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                return false;
              }
              if (true !== data1) {
                validate125.errors = [{ instancePath: instancePath + "/success", schemaPath: "#/properties/success/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
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
      validate125.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
      return false;
    }
  }
  validate125.errors = vErrors;
  return errors === 0;
}
export {
  validateAvailableVersionsOutcome,
  validateBackendStatusOutcome,
  validateCancelInstallationOutcome,
  validateCatalogSearchOutcome,
  validateCheckVersionDependenciesOutcome,
  validateCheckVersionDependenciesParams,
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
  validateGetReleaseDependenciesOutcome,
  validateGetReleaseDependenciesParams,
  validateGithubCacheStatusOutcome,
  validateHfDownloadDetailsOutcome,
  validateInferenceSettingsOutcome,
  validateInstallVersionOutcome,
  validateInstallVersionParams,
  validateInstallationProgressOutcome,
  validateInstalledVersionsOutcome,
  validateLibraryModelMetadataOutcome,
  validateLinkHealthOutcome,
  validateModelIndexRefreshOutcome,
  validateModelsOutcome,
  validatePartialDownloadOutcome,
  validatePublicError,
  validateRecoverDownloadParams,
  validateRemoveVersionOutcome,
  validateSearchCatalogParams,
  validateSelectedVersionOutcome,
  validateSetDefaultVersionOutcome,
  validateSetDefaultVersionParams,
  validateStartBackendSetupParams,
  validateStartConversionSetupParams,
  validateSuccessOutcome,
  validateSupportedQuantTypesOutcome,
  validateSwitchVersionOutcome,
  validateUpdateInferenceSettingsOutcome,
  validateUpdateInferenceSettingsParams,
  validateUpdateModelNotesOutcome,
  validateUpdateModelNotesParams,
  validateValidateInstallationsOutcome,
  validateVersionInfoOutcome,
  validateVersionStatusOutcome
};
