
  // ==========================================================================
  // Block B: Frida 16 -> 17 Compatibility Shim (Object.create wrappers)
  // ==========================================================================
  var __compatModule = Object.create(Module);
  __compatModule.findBaseAddress = function (name) {
    var m = Process.findModuleByName(name);
    return m ? m.base : null;
  };
  __compatModule.getBaseAddress = function (name) {
    return Process.getModuleByName(name).base;
  };
  __compatModule.findExportByName = function (moduleName, exportName) {
    if (moduleName === null || moduleName === undefined) {
      return typeof Module.findGlobalExportByName === 'function'
        ? Module.findGlobalExportByName(exportName)
        : null;
    }
    var m = Process.findModuleByName(moduleName);
    return m ? m.findExportByName(exportName) : null;
  };
  __compatModule.getExportByName = function (moduleName, exportName) {
    if (moduleName === null || moduleName === undefined) {
      if (typeof Module.getGlobalExportByName === 'function') {
        return Module.getGlobalExportByName(exportName);
      }
      var found = __compatModule.findExportByName(null, exportName);
      if (found === null) {
        throw new Error('unable to find global export ' + exportName);
      }
      return found;
    }
    return Process.getModuleByName(moduleName).getExportByName(exportName);
  };
  __compatModule.findSymbolByName = function (moduleName, symbolName) {
    if (!moduleName) {
      return null;
    }
    var m = Process.findModuleByName(moduleName);
    return (m && typeof m.findSymbolByName === 'function')
      ? m.findSymbolByName(symbolName)
      : null;
  };
  __compatModule.getSymbolByName = function (moduleName, symbolName) {
    var addr = __compatModule.findSymbolByName(moduleName, symbolName);
    if (addr === null) {
      throw new Error('unable to find symbol ' + symbolName + ' in ' + moduleName);
    }
    return addr;
  };
  __compatModule.ensureInitialized = function (name) {
    try {
      Process.findModuleByName(name);
    } catch (e) {}
  };

  var __compatMemory = Object.create(Memory);
  var __memSuffixes = [
    'S8', 'U8', 'S16', 'U16', 'S32', 'U32', 'S64', 'U64',
    'Short', 'UShort', 'Int', 'UInt', 'Long', 'ULong',
    'Float', 'Double', 'Pointer', 'ByteArray',
    'CString', 'Utf8String', 'Utf16String', 'AnsiString'
  ];
  for (var __mi = 0; __mi < __memSuffixes.length; __mi++) {
    (function (suffix) {
      var readName = 'read' + suffix;
      var writeName = 'write' + suffix;
      __compatMemory[readName] = function (address) {
        var p = (address instanceof NativePointer) ? address : ptr(address);
        var rest = Array.prototype.slice.call(arguments, 1);
        return p[readName].apply(p, rest);
      };
      __compatMemory[writeName] = function (address) {
        var p = (address instanceof NativePointer) ? address : ptr(address);
        var rest = Array.prototype.slice.call(arguments, 1);
        return p[writeName].apply(p, rest);
      };
    })(__memSuffixes[__mi]);
  }

  // ==========================================================================
  // Block C: Loopback NDJSON Log & Handshake Channel (+ File Fallback)
  // ==========================================================================
  var __MAX_QUEUE_ITEMS = 8192;
  var __logQueue = [];
  var __logOutput = null;
  var __logWriting = false;
  var __logFallback = false;
  var __connectAttempts = 0;
  var __MAX_CONNECT_ATTEMPTS = 5;

  function __pad2(n) {
    return n < 10 ? '0' + n : String(n);
  }

  function __formatNow() {
    var d = new Date();
    return d.getFullYear() + '-' + __pad2(d.getMonth() + 1) + '-' + __pad2(d.getDate())
      + ' ' + __pad2(d.getHours()) + ':' + __pad2(d.getMinutes()) + ':' + __pad2(d.getSeconds());
  }

  function __encodeUtf8(str) {
    var bytes = [];
    for (var i = 0; i < str.length; i++) {
      var code = str.charCodeAt(i);
      if (code < 0x80) {
        bytes.push(code);
      } else if (code < 0x800) {
        bytes.push(0xC0 | (code >> 6));
        bytes.push(0x80 | (code & 0x3F));
      } else if (code >= 0xD800 && code <= 0xDBFF) {
        if (i + 1 < str.length) {
          var low = str.charCodeAt(i + 1);
          if (low >= 0xDC00 && low <= 0xDFFF) {
            i += 1;
            var cp = 0x10000 + (((code & 0x3FF) << 10) | (low & 0x3FF));
            bytes.push(0xF0 | (cp >> 18));
            bytes.push(0x80 | ((cp >> 12) & 0x3F));
            bytes.push(0x80 | ((cp >> 6) & 0x3F));
            bytes.push(0x80 | (cp & 0x3F));
            continue;
          }
        }
        bytes.push(0xEF, 0xBF, 0xBD);
      } else if (code >= 0xDC00 && code <= 0xDFFF) {
        bytes.push(0xEF, 0xBF, 0xBD);
      } else {
        bytes.push(0xE0 | (code >> 12));
        bytes.push(0x80 | ((code >> 6) & 0x3F));
        bytes.push(0x80 | (code & 0x3F));
      }
    }
    return new Uint8Array(bytes).buffer;
  }

  function __writeFallbackEntry(obj) {
    if (!__PATCH_LOG_PATH || !obj) {
      return;
    }
    if (obj.t !== 'LOG' && obj.t !== 'INIT_ERR') {
      return;
    }
    try {
      var level = obj.l || (obj.t === 'INIT_ERR' ? 'ERROR' : 'INFO');
      var file = obj.f || 'bundle';
      var msg = obj.m || '';
      var f = new File(__PATCH_LOG_PATH, 'a');
      try {
        f.write('[' + __formatNow() + '] [' + level + '] [' + file + '] ' + msg + '\n');
      } finally {
        f.close();
      }
    } catch (e) {}
  }

  function __enterFallback() {
    __logFallback = true;
    __logOutput = null;
    __logWriting = false;
    var pending = __logQueue;
    __logQueue = [];
    for (var i = 0; i < pending.length; i++) {
      __writeFallbackEntry(pending[i]);
    }
  }

  function __flushQueue() {
    if (__logFallback || !__logOutput || __logWriting || __logQueue.length === 0) {
      return;
    }
    __logWriting = true;
    var batchObjs = __logQueue;
    __logQueue = [];
    var text = '';
    for (var i = 0; i < batchObjs.length; i++) {
      try {
        text += JSON.stringify(batchObjs[i]) + '\n';
      } catch (e) {}
    }
    if (text.length === 0) {
      __logWriting = false;
      return;
    }
    var buf = __encodeUtf8(text);
    try {
      __logOutput.writeAll(buf).then(function () {
        __logWriting = false;
        if (__logQueue.length > 0) {
          __flushQueue();
        }
      }).catch(function () {
        for (var j = 0; j < batchObjs.length; j++) {
          __writeFallbackEntry(batchObjs[j]);
        }
        __enterFallback();
      });
    } catch (e) {
      for (var k = 0; k < batchObjs.length; k++) {
        __writeFallbackEntry(batchObjs[k]);
      }
      __enterFallback();
    }
  }

  function __enqueue(obj) {
    obj.n = __SESSION_NONCE;
    if (__logFallback) {
      __writeFallbackEntry(obj);
      return;
    }
    if (__logQueue.length >= __MAX_QUEUE_ITEMS) {
      __logQueue.shift();
    }
    __logQueue.push(obj);
    __flushQueue();
  }

  function __emitStatus(type, fileName, message) {
    var packet = { t: type };
    if (fileName !== null && fileName !== undefined) {
      packet.f = String(fileName);
    }
    if (message !== null && message !== undefined) {
      packet.m = String(message);
    }
    __enqueue(packet);
  }

  function __emitLog(level, fileName, message) {
    __enqueue({
      t: 'LOG',
      l: level,
      f: String(fileName),
      m: String(message)
    });
  }

  function __safeStringify(val) {
    if (typeof val === 'string') {
      return val;
    }
    if (val === undefined) {
      return 'undefined';
    }
    if (val === null) {
      return 'null';
    }
    try {
      return JSON.stringify(val);
    } catch (e) {
      try {
        return String(val);
      } catch (e2) {
        return '[unprintable]';
      }
    }
  }

  function __formatArgs(args) {
    var parts = [];
    for (var i = 0; i < args.length; i++) {
      parts.push(__safeStringify(args[i]));
    }
    return parts.join(' ');
  }

  function __makeConsole(fileName) {
    return {
      log: function () { __emitLog('INFO', fileName, __formatArgs(arguments)); },
      info: function () { __emitLog('INFO', fileName, __formatArgs(arguments)); },
      debug: function () { __emitLog('INFO', fileName, __formatArgs(arguments)); },
      warn: function () { __emitLog('WARN', fileName, __formatArgs(arguments)); },
      error: function () { __emitLog('ERROR', fileName, __formatArgs(arguments)); }
    };
  }

  function __makeSend(fileName) {
    return function (payload) {
      __emitLog('INFO', fileName, __safeStringify(payload));
    };
  }

  function __connectLogChannel() {
    if (__logFallback || __logOutput) {
      return;
    }
    __connectAttempts += 1;
    try {
      Socket.connect({ family: 'ipv4', host: '127.0.0.1', port: __LOG_PORT })
        .then(function (conn) {
          __logOutput = conn.output;
          __flushQueue();
        })
        .catch(function () {
          if (__connectAttempts < __MAX_CONNECT_ATTEMPTS) {
            setTimeout(__connectLogChannel, 100);
          } else {
            __enterFallback();
          }
        });
    } catch (e) {
      if (__connectAttempts < __MAX_CONNECT_ATTEMPTS) {
        setTimeout(__connectLogChannel, 100);
      } else {
        __enterFallback();
      }
    }
  }

  // Schedule async loopback connection so top-level synchronous hook installation
  // finishes cleanly before any Winsock activity runs.
  setTimeout(__connectLogChannel, 0);

  // ==========================================================================
  // Block D: Isolated Per-MOD Execution Loop
  // ==========================================================================
  var __hadInitError = false;
  for (var i = 0; i < __MODS.length; i++) {
    var mod = __MODS[i];
    var modConsole = __makeConsole(mod.fileName);
    var modSend = __makeSend(mod.fileName);
    try {
      var runner = new Function('console', 'send', 'Module', 'Memory', mod.source);
      runner(modConsole, modSend, __compatModule, __compatMemory);
      __emitStatus('MOD_OK', mod.fileName, null);
    } catch (err) {
      __hadInitError = true;
      var detail = (err && err.stack) ? String(err.stack) : String(err);
      var msg = (err && err.message) ? String(err.message) : String(err);
      __emitLog('ERROR', mod.fileName, detail);
      __emitStatus('INIT_ERR', mod.fileName, msg);
      break;
    }
  }

  if (!__hadInitError) {
    __emitStatus('INIT_OK', 'bundle', null);
  }
