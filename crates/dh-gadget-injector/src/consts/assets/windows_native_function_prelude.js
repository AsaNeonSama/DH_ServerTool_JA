(function () {
  var _NF = NativeFunction;
  function _PNF(a, r, t, o) {
    var p = (o === undefined || o === null)
      ? { exceptions:"propagate" }
      : (typeof o === "string"
        ? { abi: o, exceptions:"propagate" }
        : Object.assign({ exceptions:"propagate" }, o));
    return new _NF(a, r, t, p);
  }
  _PNF.prototype = _NF.prototype;
  globalThis.NativeFunction = _PNF;
})();
