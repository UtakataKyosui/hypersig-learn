var __defProp = Object.defineProperty;
var __name = (target, value) => __defProp(target, "name", { value, configurable: true });

// build/index.js
import { WorkerEntrypoint as ve } from "cloudflare:workers";
import Z from "./ff3bc40a4b3229346b67b27c063de947683c5c97-index_bg.wasm";
var G = globalThis.__worker_init_state = { criticalError: false, instanceId: 0 };
var v = class {
  static {
    __name(this, "v");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, ue.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_containerstartupoptions_free(e, 0);
    } catch (t) {
      o(t);
    }
  }
  get enableInternet() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_containerstartupoptions_enableInternet(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return e === 16777215 ? void 0 : e !== 0;
  }
  get entrypoint() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_containerstartupoptions_entrypoint(this.__wbg_ptr);
    } catch (n) {
      o(n);
    }
    var t = le(e[0], e[1]);
    return i.__wbindgen_free(e[0], e[1] * 4, 4), t;
  }
  get env() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_containerstartupoptions_env(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return e;
  }
  set enableInternet(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_containerstartupoptions_enableInternet(this.__wbg_ptr, u(e) ? 16777215 : e ? 1 : 0);
    } catch (t) {
      o(t);
    }
  }
  set entrypoint(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let t = he(e, i.__wbindgen_malloc), n = h;
    c();
    try {
      i.__wbg_set_containerstartupoptions_entrypoint(this.__wbg_ptr, t, n);
    } catch (r) {
      o(r);
    }
  }
  set env(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_containerstartupoptions_env(this.__wbg_ptr, e);
    } catch (t) {
      o(t);
    }
  }
};
Symbol.dispose && (v.prototype[Symbol.dispose] = v.prototype.free);
var x = class {
  static {
    __name(this, "x");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, K.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_hub_free(e, 0);
    } catch (t) {
      o(t);
    }
  }
  fetch(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.hub_fetch(this.__wbg_ptr, e);
    } catch (n) {
      o(n);
    }
    return t;
  }
  constructor(e, t) {
    let n;
    c();
    try {
      n = i.hub_new(e, t);
    } catch (r) {
      o(r);
    }
    return this.__wbg_ptr = n, Object.defineProperty(this, "__wbg_inst", { value: s, writable: true }), K.register(this, { ptr: n, instance: s }, this), this;
  }
};
Symbol.dispose && (x.prototype[Symbol.dispose] = x.prototype.free);
var z = class {
  static {
    __name(this, "z");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, be.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_intounderlyingbytesource_free(e, 0);
    } catch (t) {
      o(t);
    }
  }
  get autoAllocateChunkSize() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.intounderlyingbytesource_autoAllocateChunkSize(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return e >>> 0;
  }
  cancel() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e = this.__destroy_into_raw();
    c();
    try {
      i.intounderlyingbytesource_cancel(e);
    } catch (t) {
      o(t);
    }
  }
  pull(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.intounderlyingbytesource_pull(this.__wbg_ptr, e);
    } catch (n) {
      o(n);
    }
    return t;
  }
  start(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.intounderlyingbytesource_start(this.__wbg_ptr, e);
    } catch (t) {
      o(t);
    }
  }
  get type() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.intounderlyingbytesource_type(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return ae[e];
  }
};
Symbol.dispose && (z.prototype[Symbol.dispose] = z.prototype.free);
var T = class {
  static {
    __name(this, "T");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, de.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_intounderlyingsink_free(e, 0);
    } catch (t) {
      o(t);
    }
  }
  abort(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let t = this.__destroy_into_raw(), n;
    c();
    try {
      n = i.intounderlyingsink_abort(t, e);
    } catch (r) {
      o(r);
    }
    return n;
  }
  close() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e = this.__destroy_into_raw(), t;
    c();
    try {
      t = i.intounderlyingsink_close(e);
    } catch (n) {
      o(n);
    }
    return t;
  }
  write(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.intounderlyingsink_write(this.__wbg_ptr, e);
    } catch (n) {
      o(n);
    }
    return t;
  }
};
Symbol.dispose && (T.prototype[Symbol.dispose] = T.prototype.free);
var k = class _ {
  static {
    __name(this, "_");
  }
  static __wrap(e) {
    let t = Object.create(_.prototype);
    return t.__wbg_ptr = e, Object.defineProperty(t, "__wbg_inst", { value: s, writable: true }), Q.register(t, { ptr: e, instance: s }, t), t;
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, Q.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_intounderlyingsource_free(e, 0);
    } catch (t) {
      o(t);
    }
  }
  cancel() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e = this.__destroy_into_raw();
    c();
    try {
      i.intounderlyingsource_cancel(e);
    } catch (t) {
      o(t);
    }
  }
  pull(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.intounderlyingsource_pull(this.__wbg_ptr, e);
    } catch (n) {
      o(n);
    }
    return t;
  }
};
Symbol.dispose && (k.prototype[Symbol.dispose] = k.prototype.free);
var I = class {
  static {
    __name(this, "I");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, we.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_minifyconfig_free(e, 0);
    } catch (t) {
      o(t);
    }
  }
  get css() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_minifyconfig_css(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return e !== 0;
  }
  get html() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_minifyconfig_html(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return e !== 0;
  }
  get js() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_minifyconfig_js(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return e !== 0;
  }
  set css(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_minifyconfig_css(this.__wbg_ptr, e);
    } catch (t) {
      o(t);
    }
  }
  set html(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_minifyconfig_html(this.__wbg_ptr, e);
    } catch (t) {
      o(t);
    }
  }
  set js(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_minifyconfig_js(this.__wbg_ptr, e);
    } catch (t) {
      o(t);
    }
  }
};
Symbol.dispose && (I.prototype[Symbol.dispose] = I.prototype.free);
var E = class {
  static {
    __name(this, "E");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, ge.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_r2range_free(e, 0);
    } catch (t) {
      o(t);
    }
  }
  get length() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_r2range_length(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return e[0] === 0 ? void 0 : e[1];
  }
  get offset() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_r2range_offset(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return e[0] === 0 ? void 0 : e[1];
  }
  get suffix() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_r2range_suffix(this.__wbg_ptr);
    } catch (t) {
      o(t);
    }
    return e[0] === 0 ? void 0 : e[1];
  }
  set length(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_r2range_length(this.__wbg_ptr, !u(e), u(e) ? 0 : e);
    } catch (t) {
      o(t);
    }
  }
  set offset(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_r2range_offset(this.__wbg_ptr, !u(e), u(e) ? 0 : e);
    } catch (t) {
      o(t);
    }
  }
  set suffix(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== s) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_r2range_suffix(this.__wbg_ptr, !u(e), u(e) ? 0 : e);
    } catch (t) {
      o(t);
    }
  }
};
Symbol.dispose && (E.prototype[Symbol.dispose] = E.prototype.free);
function O() {
  s++, y = null, R = null, j = null, typeof numBytesDecoded < "u" && (numBytesDecoded = 0), typeof h < "u" && (h = 0), q = false, P = false, N = new WebAssembly.Instance(Z, ne()), i = N.exports, i.__wbindgen_start();
}
__name(O, "O");
function ee() {
  let _2;
  c();
  try {
    _2 = i.__worker_init_state();
  } catch (e) {
    o(e);
  }
  return _2;
}
__name(ee, "ee");
function te(_2, e, t) {
  let n;
  c();
  try {
    n = i.fetch(_2, e, t);
  } catch (r) {
    o(r);
  }
  return n;
}
__name(te, "te");
function ne() {
  return { __proto__: null, "./index_bg.js": { __proto__: null, __wbg_Error_30c8987f7c2ed4e2: /* @__PURE__ */ __name(function(e, t) {
    return Error(l(e, t));
  }, "__wbg_Error_30c8987f7c2ed4e2"), __wbg_Number_14af1003b8dd5ead: /* @__PURE__ */ __name(function(e) {
    return Number(e);
  }, "__wbg_Number_14af1003b8dd5ead"), __wbg_String_8564e559799eccda: /* @__PURE__ */ __name(function(e, t) {
    let n = String(t), r = m(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = h;
    d().setInt32(e + 4, a, true), d().setInt32(e + 0, r, true);
  }, "__wbg_String_8564e559799eccda"), __wbg___wbindgen_boolean_get_5b446f51afd21013: /* @__PURE__ */ __name(function(e) {
    let t = e, n = typeof t == "boolean" ? t : void 0;
    return u(n) ? 16777215 : n ? 1 : 0;
  }, "__wbg___wbindgen_boolean_get_5b446f51afd21013"), __wbg___wbindgen_debug_string_4687d8d8c2017d52: /* @__PURE__ */ __name(function(e, t) {
    let n = D(t), r = m(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = h;
    d().setInt32(e + 4, a, true), d().setInt32(e + 0, r, true);
  }, "__wbg___wbindgen_debug_string_4687d8d8c2017d52"), __wbg___wbindgen_is_function_1f9d30630b8b1d3d: /* @__PURE__ */ __name(function(e) {
    return typeof e == "function";
  }, "__wbg___wbindgen_is_function_1f9d30630b8b1d3d"), __wbg___wbindgen_is_null_or_undefined_f9435d56bdcc06df: /* @__PURE__ */ __name(function(e) {
    return e == null;
  }, "__wbg___wbindgen_is_null_or_undefined_f9435d56bdcc06df"), __wbg___wbindgen_is_undefined_8865fb403f8fe9d8: /* @__PURE__ */ __name(function(e) {
    return e === void 0;
  }, "__wbg___wbindgen_is_undefined_8865fb403f8fe9d8"), __wbg___wbindgen_jsval_loose_eq_677f21e468d6b461: /* @__PURE__ */ __name(function(e, t) {
    return e == t;
  }, "__wbg___wbindgen_jsval_loose_eq_677f21e468d6b461"), __wbg___wbindgen_number_get_2e0e7dee9f701a71: /* @__PURE__ */ __name(function(e, t) {
    let n = t, r = typeof n == "number" ? n : void 0;
    d().setFloat64(e + 8, u(r) ? 0 : r, true), d().setInt32(e + 0, !u(r), true);
  }, "__wbg___wbindgen_number_get_2e0e7dee9f701a71"), __wbg___wbindgen_reinit_3f79fdb34dee3ea9: /* @__PURE__ */ __name(function() {
    P = true;
  }, "__wbg___wbindgen_reinit_3f79fdb34dee3ea9"), __wbg___wbindgen_string_get_0380ccaa2f57f0d9: /* @__PURE__ */ __name(function(e, t) {
    let n = t, r = typeof n == "string" ? n : void 0;
    var a = u(r) ? 0 : m(r, i.__wbindgen_malloc, i.__wbindgen_realloc), f = h;
    d().setInt32(e + 4, f, true), d().setInt32(e + 0, a, true);
  }, "__wbg___wbindgen_string_get_0380ccaa2f57f0d9"), __wbg___wbindgen_throw_41e9ee4f547fc59a: /* @__PURE__ */ __name(function(e, t) {
    throw new WebAssembly.Exception(J, [new Error(l(e, t))]);
  }, "__wbg___wbindgen_throw_41e9ee4f547fc59a"), __wbg__wbg_cb_unref_dcc1a90847f04c41: /* @__PURE__ */ __name(function(e) {
    e._wbg_cb_unref();
  }, "__wbg__wbg_cb_unref_dcc1a90847f04c41"), __wbg_append_1938ed58fb5613b8: /* @__PURE__ */ __name(function(e, t, n, r, a) {
    e.append(l(t, n), l(r, a));
  }, "__wbg_append_1938ed58fb5613b8"), __wbg_bind_1f842289d2018ed8: /* @__PURE__ */ __name(function(e, t) {
    return e.bind(...t);
  }, "__wbg_bind_1f842289d2018ed8"), __wbg_body_4792166cb351f162: /* @__PURE__ */ __name(function(e) {
    let t = e.body;
    return u(t) ? 0 : w(t);
  }, "__wbg_body_4792166cb351f162"), __wbg_body_4e088733babc630d: /* @__PURE__ */ __name(function(e) {
    let t = e.body;
    return u(t) ? 0 : w(t);
  }, "__wbg_body_4e088733babc630d"), __wbg_buffer_56ec2905a66f58b9: /* @__PURE__ */ __name(function(e) {
    return e.buffer;
  }, "__wbg_buffer_56ec2905a66f58b9"), __wbg_byobRequest_54f7ce41d584549a: /* @__PURE__ */ __name(function(e) {
    let t = e.byobRequest;
    return u(t) ? 0 : w(t);
  }, "__wbg_byobRequest_54f7ce41d584549a"), __wbg_byteLength_9f985b8373f1f3fe: /* @__PURE__ */ __name(function(e) {
    return e.byteLength;
  }, "__wbg_byteLength_9f985b8373f1f3fe"), __wbg_byteOffset_0bd5a6a03553639c: /* @__PURE__ */ __name(function(e) {
    return e.byteOffset;
  }, "__wbg_byteOffset_0bd5a6a03553639c"), __wbg_call_187d372bd5fdd4aa: /* @__PURE__ */ __name(function(e, t, n) {
    return e.call(t, n);
  }, "__wbg_call_187d372bd5fdd4aa"), __wbg_cancel_7c2a08c8db4c73f3: /* @__PURE__ */ __name(function(e) {
    return e.cancel();
  }, "__wbg_cancel_7c2a08c8db4c73f3"), __wbg_catch_92594c2d17f55f12: /* @__PURE__ */ __name(function(e, t) {
    return e.catch(t);
  }, "__wbg_catch_92594c2d17f55f12"), __wbg_cause_19276d032a32d4bd: /* @__PURE__ */ __name(function(e) {
    return e.cause;
  }, "__wbg_cause_19276d032a32d4bd"), __wbg_cf_909bcfb212a8822c: /* @__PURE__ */ __name(function(e) {
    let t = e.cf;
    return u(t) ? 0 : w(t);
  }, "__wbg_cf_909bcfb212a8822c"), __wbg_cf_b47fe5ff6d015a0d: /* @__PURE__ */ __name(function(e) {
    let t = e.cf;
    return u(t) ? 0 : w(t);
  }, "__wbg_cf_b47fe5ff6d015a0d"), __wbg_clearTimeout_d49a4afcd9b218b7: /* @__PURE__ */ __name(function(e, t) {
    e.clearTimeout(t);
  }, "__wbg_clearTimeout_d49a4afcd9b218b7"), __wbg_close_185286988f30f252: /* @__PURE__ */ __name(function(e) {
    e.close();
  }, "__wbg_close_185286988f30f252"), __wbg_close_748f26698dabe269: /* @__PURE__ */ __name(function(e) {
    e.close();
  }, "__wbg_close_748f26698dabe269"), __wbg_constructor_076176ce0f386e16: /* @__PURE__ */ __name(function(e) {
    return e.constructor;
  }, "__wbg_constructor_076176ce0f386e16"), __wbg_done_b41a1d26cdb37fb6: /* @__PURE__ */ __name(function(e) {
    return e.done;
  }, "__wbg_done_b41a1d26cdb37fb6"), __wbg_enqueue_4a01c128f9a0de16: /* @__PURE__ */ __name(function(e, t) {
    e.enqueue(t);
  }, "__wbg_enqueue_4a01c128f9a0de16"), __wbg_entries_b55ec7e0dc443355: /* @__PURE__ */ __name(function(e) {
    return e.entries();
  }, "__wbg_entries_b55ec7e0dc443355"), __wbg_error_a30e98d44009f1bb: /* @__PURE__ */ __name(function(e, t) {
    console.error(e, t);
  }, "__wbg_error_a30e98d44009f1bb"), __wbg_error_c9cf3fc2064683a9: /* @__PURE__ */ __name(function(e) {
    console.error(e);
  }, "__wbg_error_c9cf3fc2064683a9"), __wbg_exec_5113c7d26c7e195b: /* @__PURE__ */ __name(function(e, t, n) {
    return e.exec(l(t, n));
  }, "__wbg_exec_5113c7d26c7e195b"), __wbg_fetch_feccdd6b31467948: /* @__PURE__ */ __name(function(e, t, n) {
    return e.fetch(l(t, n));
  }, "__wbg_fetch_feccdd6b31467948"), __wbg_first_3f7220e4ae4ce878: /* @__PURE__ */ __name(function(e, t, n) {
    return e.first(t === 0 ? void 0 : l(t, n));
  }, "__wbg_first_3f7220e4ae4ce878"), __wbg_getByName_88b6d54123310eb8: /* @__PURE__ */ __name(function(e, t, n) {
    return e.getByName(l(t, n));
  }, "__wbg_getByName_88b6d54123310eb8"), __wbg_getReader_bf34f92ee19c42b4: /* @__PURE__ */ __name(function(e) {
    return e.getReader();
  }, "__wbg_getReader_bf34f92ee19c42b4"), __wbg_get_31af05bd4842a84f: /* @__PURE__ */ __name(function(e, t) {
    return Reflect.get(e, t);
  }, "__wbg_get_31af05bd4842a84f"), __wbg_get_6c896e0571ddae51: /* @__PURE__ */ __name(function(e, t) {
    return e[t >>> 0];
  }, "__wbg_get_6c896e0571ddae51"), __wbg_get_88983e33467eae60: /* @__PURE__ */ __name(function(e, t) {
    let n = Reflect.get(e, t);
    return u(n) ? 0 : w(n);
  }, "__wbg_get_88983e33467eae60"), __wbg_get_done_7020b91c987365c7: /* @__PURE__ */ __name(function(e) {
    let t = e.done;
    return u(t) ? 16777215 : t ? 1 : 0;
  }, "__wbg_get_done_7020b91c987365c7"), __wbg_get_value_87fbd6ad1283f2e7: /* @__PURE__ */ __name(function(e) {
    return e.value;
  }, "__wbg_get_value_87fbd6ad1283f2e7"), __wbg_headers_6930a8bd09b630de: /* @__PURE__ */ __name(function(e) {
    return e.headers;
  }, "__wbg_headers_6930a8bd09b630de"), __wbg_headers_eba93595f8944c2f: /* @__PURE__ */ __name(function(e) {
    return e.headers;
  }, "__wbg_headers_eba93595f8944c2f"), __wbg_httpProtocol_f9800df7fe5afb9c: /* @__PURE__ */ __name(function(e, t) {
    let n = t.httpProtocol, r = m(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = h;
    d().setInt32(e + 4, a, true), d().setInt32(e + 0, r, true);
  }, "__wbg_httpProtocol_f9800df7fe5afb9c"), __wbg_instanceId_9b67ff33ae8bef8a: /* @__PURE__ */ __name(function(e) {
    return e.instanceId;
  }, "__wbg_instanceId_9b67ff33ae8bef8a"), __wbg_instanceof_ArrayBuffer_a99f175873e5d9b8: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof ArrayBuffer;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_ArrayBuffer_a99f175873e5d9b8"), __wbg_instanceof_Error_80a725f81f2e102d: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof Error;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_Error_80a725f81f2e102d"), __wbg_instanceof_ReadableStream_e2d77912575119ab: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof ReadableStream;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_ReadableStream_e2d77912575119ab"), __wbg_instanceof_Response_b8758567269c30b2: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof Response;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_Response_b8758567269c30b2"), __wbg_instanceof_Uint8Array_828cef2aaacafc31: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof Uint8Array;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_Uint8Array_828cef2aaacafc31"), __wbg_isSafeInteger_717808ad6a54bd9e: /* @__PURE__ */ __name(function(e) {
    return Number.isSafeInteger(e);
  }, "__wbg_isSafeInteger_717808ad6a54bd9e"), __wbg_length_7f3c00c40364105e: /* @__PURE__ */ __name(function(e) {
    return e.length;
  }, "__wbg_length_7f3c00c40364105e"), __wbg_message_5f8387f0c32b90a7: /* @__PURE__ */ __name(function(e) {
    return e.message;
  }, "__wbg_message_5f8387f0c32b90a7"), __wbg_method_6d02ee0808a7de31: /* @__PURE__ */ __name(function(e, t) {
    let n = t.method, r = m(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = h;
    d().setInt32(e + 4, a, true), d().setInt32(e + 0, r, true);
  }, "__wbg_method_6d02ee0808a7de31"), __wbg_name_e2eac7cdfa054f65: /* @__PURE__ */ __name(function(e) {
    return e.name;
  }, "__wbg_name_e2eac7cdfa054f65"), __wbg_name_f9b814d78e8e299f: /* @__PURE__ */ __name(function(e) {
    return e.name;
  }, "__wbg_name_f9b814d78e8e299f"), __wbg_new_1dbf7428bba60a42: /* @__PURE__ */ __name(function(e) {
    return new Uint8Array(e);
  }, "__wbg_new_1dbf7428bba60a42"), __wbg_new_343a093a3c2ffb4e: /* @__PURE__ */ __name(function(e, t) {
    return new Error(l(e, t));
  }, "__wbg_new_343a093a3c2ffb4e"), __wbg_new_617a8cdb8bb1130e: /* @__PURE__ */ __name(function() {
    return new Object();
  }, "__wbg_new_617a8cdb8bb1130e"), __wbg_new_83ca405eae412a62: /* @__PURE__ */ __name(function() {
    return new Headers();
  }, "__wbg_new_83ca405eae412a62"), __wbg_new_ee2291f50781bf1d: /* @__PURE__ */ __name(function() {
    return new Array();
  }, "__wbg_new_ee2291f50781bf1d"), __wbg_new_typed_b01cb72a8af741a3: /* @__PURE__ */ __name(function(e, t) {
    try {
      var n = { a: e, b: t }, r = /* @__PURE__ */ __name((f, b) => {
        let g = n.a;
        n.a = 0;
        try {
          return ce(g, n.b, f, b);
        } finally {
          n.a = g;
        }
      }, "r");
      return new Promise(r);
    } finally {
      n.a = 0;
    }
  }, "__wbg_new_typed_b01cb72a8af741a3"), __wbg_new_with_byte_offset_and_length_2f5d7fc2a828b74d: /* @__PURE__ */ __name(function(e, t, n) {
    return new Uint8Array(e, t >>> 0, n >>> 0);
  }, "__wbg_new_with_byte_offset_and_length_2f5d7fc2a828b74d"), __wbg_new_with_into_underlying_source_a57cfcc7c705351c: /* @__PURE__ */ __name(function(e, t) {
    return new ReadableStream(k.__wrap(e), t);
  }, "__wbg_new_with_into_underlying_source_a57cfcc7c705351c"), __wbg_new_with_length_3da0ad195f6f63ba: /* @__PURE__ */ __name(function(e) {
    return new Uint8Array(e >>> 0);
  }, "__wbg_new_with_length_3da0ad195f6f63ba"), __wbg_new_with_opt_buffer_source_and_init_c1b859a23c80afca: /* @__PURE__ */ __name(function(e, t) {
    return new Response(e, t);
  }, "__wbg_new_with_opt_buffer_source_and_init_c1b859a23c80afca"), __wbg_new_with_opt_readable_stream_and_init_df114f2d364f216d: /* @__PURE__ */ __name(function(e, t) {
    return new Response(e, t);
  }, "__wbg_new_with_opt_readable_stream_and_init_df114f2d364f216d"), __wbg_new_with_opt_str_and_init_33bef1b272950372: /* @__PURE__ */ __name(function(e, t, n) {
    return new Response(e === 0 ? void 0 : l(e, t), n);
  }, "__wbg_new_with_opt_str_and_init_33bef1b272950372"), __wbg_next_f4aac29c42af995c: /* @__PURE__ */ __name(function(e) {
    return e.next();
  }, "__wbg_next_f4aac29c42af995c"), __wbg_now_aa4ccb83129e9e55: /* @__PURE__ */ __name(function() {
    return Date.now();
  }, "__wbg_now_aa4ccb83129e9e55"), __wbg_prepare_6fe68a8990920f81: /* @__PURE__ */ __name(function(e, t, n) {
    return e.prepare(l(t, n));
  }, "__wbg_prepare_6fe68a8990920f81"), __wbg_prototypesetcall_bc27214492979395: /* @__PURE__ */ __name(function(e, t, n) {
    Uint8Array.prototype.set.call(Y(e, t), n);
  }, "__wbg_prototypesetcall_bc27214492979395"), __wbg_push_2baf45db356cf468: /* @__PURE__ */ __name(function(e, t) {
    return e.push(t);
  }, "__wbg_push_2baf45db356cf468"), __wbg_queueMicrotask_9833f9a49df95a49: /* @__PURE__ */ __name(function(e) {
    return e.queueMicrotask;
  }, "__wbg_queueMicrotask_9833f9a49df95a49"), __wbg_queueMicrotask_a72f977e97f23c5f: /* @__PURE__ */ __name(function(e) {
    queueMicrotask(e);
  }, "__wbg_queueMicrotask_a72f977e97f23c5f"), __wbg_read_2a943774344c9728: /* @__PURE__ */ __name(function(e) {
    return e.read();
  }, "__wbg_read_2a943774344c9728"), __wbg_redirect_fba20b35e50b1511: /* @__PURE__ */ __name(function(e) {
    let t = e.redirect;
    return (fe.indexOf(t) + 1 || 4) - 1;
  }, "__wbg_redirect_fba20b35e50b1511"), __wbg_releaseLock_eb64cd83aa9fcfe4: /* @__PURE__ */ __name(function(e) {
    e.releaseLock();
  }, "__wbg_releaseLock_eb64cd83aa9fcfe4"), __wbg_resolve_0076e10020304ede: /* @__PURE__ */ __name(function(e) {
    return Promise.resolve(e);
  }, "__wbg_resolve_0076e10020304ede"), __wbg_respond_c102fabcc79e5ef0: /* @__PURE__ */ __name(function(e, t) {
    e.respond(t >>> 0);
  }, "__wbg_respond_c102fabcc79e5ef0"), __wbg_run_0ef9f3480a049b01: /* @__PURE__ */ __name(function(e) {
    return e.run();
  }, "__wbg_run_0ef9f3480a049b01"), __wbg_setTimeout_7fb1e1fe294b6ad9: /* @__PURE__ */ __name(function(e, t, n) {
    return e.setTimeout(t, n);
  }, "__wbg_setTimeout_7fb1e1fe294b6ad9"), __wbg_set_145a351398b48c65: /* @__PURE__ */ __name(function(e, t, n) {
    return Reflect.set(e, t, n);
  }, "__wbg_set_145a351398b48c65"), __wbg_set_575d3ddb70fe831d: /* @__PURE__ */ __name(function(e, t, n) {
    e.set(Y(t, n));
  }, "__wbg_set_575d3ddb70fe831d"), __wbg_set_criticalError_3904d60fe2a0b05b: /* @__PURE__ */ __name(function(e, t) {
    e.criticalError = t !== 0;
  }, "__wbg_set_criticalError_3904d60fe2a0b05b"), __wbg_set_headers_69ef7c46a2e6c742: /* @__PURE__ */ __name(function(e, t) {
    e.headers = t;
  }, "__wbg_set_headers_69ef7c46a2e6c742"), __wbg_set_high_water_mark_3a96edf83df7e06c: /* @__PURE__ */ __name(function(e, t) {
    e.highWaterMark = t;
  }, "__wbg_set_high_water_mark_3a96edf83df7e06c"), __wbg_set_instanceId_9ad5caac0a4e519c: /* @__PURE__ */ __name(function(e, t) {
    e.instanceId = t >>> 0;
  }, "__wbg_set_instanceId_9ad5caac0a4e519c"), __wbg_set_status_678f122cbe435d95: /* @__PURE__ */ __name(function(e, t) {
    e.status = t;
  }, "__wbg_set_status_678f122cbe435d95"), __wbg_signal_26a6bdfa17b46df6: /* @__PURE__ */ __name(function(e) {
    return e.signal;
  }, "__wbg_signal_26a6bdfa17b46df6"), __wbg_static_accessor_GLOBAL_266715b9d96ba635: /* @__PURE__ */ __name(function() {
    let e = typeof global > "u" ? null : global;
    return u(e) ? 0 : w(e);
  }, "__wbg_static_accessor_GLOBAL_266715b9d96ba635"), __wbg_static_accessor_GLOBAL_THIS_10fb7dc1ae063179: /* @__PURE__ */ __name(function() {
    let e = typeof globalThis > "u" ? null : globalThis;
    return u(e) ? 0 : w(e);
  }, "__wbg_static_accessor_GLOBAL_THIS_10fb7dc1ae063179"), __wbg_static_accessor_INIT_STATE_e3fce2ad8a08e94e: /* @__PURE__ */ __name(function() {
    return G;
  }, "__wbg_static_accessor_INIT_STATE_e3fce2ad8a08e94e"), __wbg_static_accessor_SELF_0b583911f537483a: /* @__PURE__ */ __name(function() {
    let e = typeof self > "u" ? null : self;
    return u(e) ? 0 : w(e);
  }, "__wbg_static_accessor_SELF_0b583911f537483a"), __wbg_static_accessor_WINDOW_d7f903d1508cbdc4: /* @__PURE__ */ __name(function() {
    let e = typeof window > "u" ? null : window;
    return u(e) ? 0 : w(e);
  }, "__wbg_static_accessor_WINDOW_d7f903d1508cbdc4"), __wbg_status_ce0a98d3c57125f3: /* @__PURE__ */ __name(function(e) {
    return e.status;
  }, "__wbg_status_ce0a98d3c57125f3"), __wbg_then_c949d5a25a4e78f8: /* @__PURE__ */ __name(function(e, t, n) {
    return e.then(t, n);
  }, "__wbg_then_c949d5a25a4e78f8"), __wbg_then_e71170d78fcf8954: /* @__PURE__ */ __name(function(e, t) {
    return e.then(t);
  }, "__wbg_then_e71170d78fcf8954"), __wbg_url_7bfada5c2297bc70: /* @__PURE__ */ __name(function(e, t) {
    let n = t.url, r = m(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = h;
    d().setInt32(e + 4, a, true), d().setInt32(e + 0, r, true);
  }, "__wbg_url_7bfada5c2297bc70"), __wbg_value_f3c585ee8f5ba40c: /* @__PURE__ */ __name(function(e) {
    return e.value;
  }, "__wbg_value_f3c585ee8f5ba40c"), __wbg_view_9c570f33e8d6ab96: /* @__PURE__ */ __name(function(e) {
    let t = e.view;
    return u(t) ? 0 : w(t);
  }, "__wbg_view_9c570f33e8d6ab96"), __wbg_webSocket_2b7e8e9549bed673: /* @__PURE__ */ __name(function(e) {
    let t = e.webSocket;
    return u(t) ? 0 : w(t);
  }, "__wbg_webSocket_2b7e8e9549bed673"), __wbindgen_generic_0000000000000001: /* @__PURE__ */ __name(function(e, t) {
    return C(e, t, se);
  }, "__wbindgen_generic_0000000000000001"), __wbindgen_generic_0000000000000002: /* @__PURE__ */ __name(function(e, t) {
    return C(e, t, oe);
  }, "__wbindgen_generic_0000000000000002"), __wbindgen_generic_0000000000000003: /* @__PURE__ */ __name(function(e, t) {
    return C(e, t, ie);
  }, "__wbindgen_generic_0000000000000003"), __wbindgen_generic_0000000000000004: /* @__PURE__ */ __name(function(e) {
    return e;
  }, "__wbindgen_generic_0000000000000004"), __wbindgen_generic_0000000000000005: /* @__PURE__ */ __name(function(e, t) {
    return l(e, t);
  }, "__wbindgen_generic_0000000000000005"), __wbindgen_init_externref_table: /* @__PURE__ */ __name(function() {
    let e = i.__wbindgen_externrefs, t = e.grow(4);
    e.set(0, void 0), e.set(t + 0, void 0), e.set(t + 1, null), e.set(t + 2, true), e.set(t + 3, false);
  }, "__wbindgen_init_externref_table"), __wbindgen_jstag: WebAssembly.JSTag, __wbindgen_rethrow_critical: /* @__PURE__ */ __name(function(e) {
    throw new Error("Critical error", { cause: e });
  }, "__wbindgen_rethrow_critical") } };
}
__name(ne, "ne");
var J = new WebAssembly.Tag({ parameters: ["externref"] });
var V;
var q = false;
function _e() {
  q = true;
  try {
    let _2 = B()[i.__abort_handler.value / 4];
    _2 && i.__wbindgen_export.get(_2)();
  } catch {
  }
}
__name(_e, "_e");
function o(_2) {
  throw _2 instanceof WebAssembly.Exception && _2.is(J) ? _2.getArg(J, 0) : (B()[V] = 1, _e(), _2);
}
__name(o, "o");
function c() {
  if (V ??= i.__instance_terminated.value / 4, B()[V]) {
    if (q || _e(), P) {
      O();
      return;
    }
    throw new Error("Module terminated");
  } else P && O();
}
__name(c, "c");
function ie(_2, e) {
  c();
  try {
    i.wasm_bindgen_a2f60164f7d35a44___convert__closures_____invoke_______true_(_2, e);
  } catch (t) {
    o(t);
  }
}
__name(ie, "ie");
function se(_2, e, t) {
  c();
  try {
    i.wasm_bindgen_a2f60164f7d35a44___convert__closures_____invoke___wasm_bindgen_a2f60164f7d35a44___JsValue______true_(_2, e, t);
  } catch (n) {
    o(n);
  }
}
__name(se, "se");
function oe(_2, e, t) {
  let n;
  c();
  try {
    n = i.wasm_bindgen_a2f60164f7d35a44___convert__closures_____invoke___wasm_bindgen_a2f60164f7d35a44___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_a2f60164f7d35a44___JsError___true_(_2, e, t);
  } catch (r) {
    o(r);
  }
  if (n[1]) throw pe(n[0]);
}
__name(oe, "oe");
function ce(_2, e, t, n) {
  c();
  try {
    i.wasm_bindgen_a2f60164f7d35a44___convert__closures_____invoke___js_sys_b702ab54a9991700___Function_fn_wasm_bindgen_a2f60164f7d35a44___JsValue_____wasm_bindgen_a2f60164f7d35a44___sys__Undefined___js_sys_b702ab54a9991700___Function_fn_wasm_bindgen_a2f60164f7d35a44___JsValue_____wasm_bindgen_a2f60164f7d35a44___sys__Undefined_______true_(_2, e, t, n);
  } catch (r) {
    o(r);
  }
}
__name(ce, "ce");
var ae = ["bytes"];
var fe = ["follow", "error", "manual"];
var s = 0;
var ue = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: _2, instance: e }) => {
  e === s && i.__wbg_containerstartupoptions_free(_2, 1);
});
var K = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: _2, instance: e }) => {
  e === s && i.__wbg_hub_free(_2, 1);
});
var be = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: _2, instance: e }) => {
  e === s && i.__wbg_intounderlyingbytesource_free(_2, 1);
});
var de = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: _2, instance: e }) => {
  e === s && i.__wbg_intounderlyingsink_free(_2, 1);
});
var Q = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: _2, instance: e }) => {
  e === s && i.__wbg_intounderlyingsource_free(_2, 1);
});
var we = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: _2, instance: e }) => {
  e === s && i.__wbg_minifyconfig_free(_2, 1);
});
var ge = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: _2, instance: e }) => {
  e === s && i.__wbg_r2range_free(_2, 1);
});
function w(_2) {
  let e = i.__externref_table_alloc();
  return i.__wbindgen_externrefs.set(e, _2), e;
}
__name(w, "w");
var X = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry((_2) => {
  _2.instance === s && i.__wbindgen_destroy_closure(_2.a, _2.b);
});
function D(_2) {
  let e = typeof _2;
  if (e == "number" || e == "boolean" || _2 == null) return `${_2}`;
  if (e == "string") return `"${_2}"`;
  if (e == "symbol") {
    let r = _2.description;
    return r == null ? "Symbol" : `Symbol(${r})`;
  }
  if (e == "function") {
    let r = _2.name;
    return typeof r == "string" && r.length > 0 ? `Function(${r})` : "Function";
  }
  if (Array.isArray(_2)) {
    let r = _2.length, a = "[";
    r > 0 && (a += D(_2[0]));
    for (let f = 1; f < r; f++) a += ", " + D(_2[f]);
    return a += "]", a;
  }
  let t = /\[object ([^\]]+)\]/.exec(toString.call(_2)), n;
  if (t && t.length > 1) n = t[1];
  else return toString.call(_2);
  if (n == "Object") try {
    return "Object(" + JSON.stringify(_2) + ")";
  } catch {
    return "Object";
  }
  return _2 instanceof Error ? `${_2.name}: ${_2.message}
${_2.stack}` : n;
}
__name(D, "D");
function le(_2, e) {
  _2 = _2 >>> 0;
  let t = d(), n = [];
  for (let r = _2; r < _2 + 4 * e; r += 4) n.push(i.__wbindgen_externrefs.get(t.getUint32(r, true)));
  return i.__externref_drop_slice(_2, e), n;
}
__name(le, "le");
function Y(_2, e) {
  return _2 = _2 >>> 0, W().subarray(_2 / 1, _2 / 1 + e);
}
__name(Y, "Y");
var y = null;
function d() {
  return (y === null || y.buffer.detached === true || y.buffer.detached === void 0 && y.buffer !== i.memory.buffer) && (y = new DataView(i.memory.buffer)), y;
}
__name(d, "d");
var R = null;
function B() {
  return (R === null || R.byteLength === 0) && (R = new Int32Array(i.memory.buffer)), R;
}
__name(B, "B");
function l(_2, e) {
  return ye(_2 >>> 0, e);
}
__name(l, "l");
var j = null;
function W() {
  return (j === null || j.byteLength === 0) && (j = new Uint8Array(i.memory.buffer)), j;
}
__name(W, "W");
function u(_2) {
  return _2 == null;
}
__name(u, "u");
function C(_2, e, t) {
  let n = { a: _2, b: e, cnt: 1, instance: s }, r = /* @__PURE__ */ __name((...a) => {
    if (n.instance !== s) throw new Error("Cannot invoke closure from previous WASM instance");
    n.cnt++;
    let f = n.a;
    n.a = 0;
    try {
      return t(f, n.b, ...a);
    } finally {
      n.a = f, r._wbg_cb_unref();
    }
  }, "r");
  return r._wbg_cb_unref = () => {
    --n.cnt === 0 && (i.__wbindgen_destroy_closure(n.a, n.b), n.a = 0, X.unregister(n));
  }, X.register(r, n, n), r;
}
__name(C, "C");
function he(_2, e) {
  let t = e(_2.length * 4, 4) >>> 0;
  for (let n = 0; n < _2.length; n++) {
    let r = w(_2[n]);
    d().setUint32(t + 4 * n, r, true);
  }
  return h = _2.length, t;
}
__name(he, "he");
function m(_2, e, t) {
  if (t === void 0) {
    let b = S.encode(_2), g = e(b.length, 1) >>> 0;
    return W().subarray(g, g + b.length).set(b), h = b.length, g;
  }
  let n = _2.length, r = e(n, 1) >>> 0, a = W(), f = 0;
  for (; f < n; f++) {
    let b = _2.charCodeAt(f);
    if (b > 127) break;
    a[r + f] = b;
  }
  if (f !== n) {
    f !== 0 && (_2 = _2.slice(f)), r = t(r, n, n = f + _2.length * 3, 1) >>> 0;
    let b = W().subarray(r + f, r + n), g = S.encodeInto(_2, b);
    f += g.written, r = t(r, n, f, 1) >>> 0;
  }
  return h = f, r;
}
__name(m, "m");
var P = false;
function pe(_2) {
  let e = i.__wbindgen_externrefs.get(_2);
  return i.__externref_table_dealloc(_2), e;
}
__name(pe, "pe");
var re = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
re.decode();
function ye(_2, e) {
  return re.decode(W().subarray(_2, _2 + e));
}
__name(ye, "ye");
var S = new TextEncoder();
"encodeInto" in S || (S.encodeInto = function(_2, e) {
  let t = S.encode(_2);
  return e.set(t), { read: _2.length, written: t.length };
});
var h = 0;
var N = new WebAssembly.Instance(Z, ne());
var i = N.exports;
i.__wbindgen_start();
Error.stackTraceLimit = 100;
var p = ee();
function $() {
  p.criticalError && (console.log("Reinitializing Wasm application"), O(), p.criticalError = false, p.instanceId++);
}
__name($, "$");
addEventListener("error", (_2) => {
  H(_2.error);
});
function H(_2) {
  _2 instanceof WebAssembly.RuntimeError && (console.error("Critical", _2), p.criticalError = true);
}
__name(H, "H");
var M = class extends ve {
  static {
    __name(this, "M");
  }
};
M.prototype.fetch = function(e) {
  return te.call(this, e, this.env, this.ctx);
};
var xe = { set: /* @__PURE__ */ __name((_2, e, t, n) => Reflect.set(_2.instance, e, t, n), "set"), has: /* @__PURE__ */ __name((_2, e) => Reflect.has(_2.instance, e), "has"), deleteProperty: /* @__PURE__ */ __name((_2, e) => Reflect.deleteProperty(_2.instance, e), "deleteProperty"), apply: /* @__PURE__ */ __name((_2, e, t) => Reflect.apply(_2.instance, e, t), "apply"), construct: /* @__PURE__ */ __name((_2, e, t) => Reflect.construct(_2.instance, e, t), "construct"), getPrototypeOf: /* @__PURE__ */ __name((_2) => Reflect.getPrototypeOf(_2.instance), "getPrototypeOf"), setPrototypeOf: /* @__PURE__ */ __name((_2, e) => Reflect.setPrototypeOf(_2.instance, e), "setPrototypeOf"), isExtensible: /* @__PURE__ */ __name((_2) => Reflect.isExtensible(_2.instance), "isExtensible"), preventExtensions: /* @__PURE__ */ __name((_2) => Reflect.preventExtensions(_2.instance), "preventExtensions"), getOwnPropertyDescriptor: /* @__PURE__ */ __name((_2, e) => Reflect.getOwnPropertyDescriptor(_2.instance, e), "getOwnPropertyDescriptor"), defineProperty: /* @__PURE__ */ __name((_2, e, t) => Reflect.defineProperty(_2.instance, e, t), "defineProperty"), ownKeys: /* @__PURE__ */ __name((_2) => Reflect.ownKeys(_2.instance), "ownKeys") };
var A = { construct(_2, e, t) {
  try {
    $();
    let n = { instance: Reflect.construct(_2, e, t), instanceId: p.instanceId, ctor: _2, args: e, newTarget: t };
    return new Proxy(n, { ...xe, get(r, a, f) {
      r.instanceId !== p.instanceId && (r.instance = Reflect.construct(r.ctor, r.args, r.newTarget), r.instanceId = p.instanceId);
      let b = Reflect.get(r.instance, a, f);
      return typeof b != "function" ? b : b.constructor === Function ? new Proxy(b, { apply(g, U, L) {
        $();
        try {
          return g.apply(U, L);
        } catch (F) {
          throw H(F), F;
        }
      } }) : new Proxy(b, { async apply(g, U, L) {
        $();
        try {
          return await g.apply(U, L);
        } catch (F) {
          throw H(F), F;
        }
      } });
    } });
  } catch (n) {
    throw p.criticalError = true, n;
  }
} };
var je = new Proxy(M, A);
var We = new Proxy(v, A);
var Se = new Proxy(x, A);
var ke = new Proxy(I, A);
var Ae = new Proxy(E, A);
export {
  We as ContainerStartupOptions,
  Se as Hub,
  ke as MinifyConfig,
  Ae as R2Range,
  je as default
};
//# sourceMappingURL=shim.js.map
