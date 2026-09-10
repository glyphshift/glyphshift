'use strict';

const installed = new WeakSet();

// Contract prototype only: the caller must already be in the game's JS realm.
// This does not bootstrap NW.js or constitute a shipping adapter.
function installMessageAdapter(WindowMessage, host) {
  const prototype = WindowMessage && WindowMessage.prototype;
  if (!prototype || typeof prototype.startMessage !== 'function' ||
      !host || typeof host.resolve !== 'function') {
    throw new Error('Unsupported message entry or missing runtime callbacks');
  }
  const descriptor = Object.getOwnPropertyDescriptor(prototype, 'startMessage');
  if (!descriptor || !descriptor.writable || typeof descriptor.value !== 'function') {
    throw new Error('Message entry is not an owned writable method');
  }
  if (installed.has(prototype)) throw new Error('Message adapter is already installed');
  const original = descriptor.value;
  let enabled = true;
  let depth = 0;

  function startMessage(...args) {
    if (!enabled || depth !== 0) return original.apply(this, args);
    const own = Object.getOwnPropertyDescriptor(this, 'convertEscapeCharacters');
    const convert = this.convertEscapeCharacters;
    // Never overwrite accessors, locked properties or somebody else's hooks.
    if (typeof convert !== 'function' ||
        (own && (!own.configurable || !Object.prototype.hasOwnProperty.call(own, 'value'))) ||
        (!own && !Object.isExtensible(this))) return original.apply(this, args);
    const receiver = this;
    let consumed = false;
    function convertMessage(source, ...rest) {
      let output = source;
      if (this === receiver && !consumed && validText(source)) {
        consumed = true;
        // Runtime decide_text already captures once before lookup. Do not issue
        // separate observe + decide calls, which would count this display twice.
        const usage = plainText(source) ? 'draw' : 'observe';
        try {
          const candidate = host.resolve({ source, origin: 'display-entry', usage });
          if (enabled && usage === 'draw' && validText(candidate) && plainText(candidate)) output = candidate;
        } catch (error) {}
      }
      return convert.call(this, output, ...rest);
    }
    Object.defineProperty(this, 'convertEscapeCharacters', {
      value: convertMessage, configurable: true, writable: true,
      enumerable: own ? own.enumerable : false,
    });
    depth++;
    try {
      return original.apply(this, args);
    } finally {
      depth--;
      const current = Object.getOwnPropertyDescriptor(this, 'convertEscapeCharacters');
      if (current && current.value === convertMessage) {
        if (own) Object.defineProperty(this, 'convertEscapeCharacters', own);
        else delete this.convertEscapeCharacters;
      }
    }
  }
  Object.defineProperty(prototype, 'startMessage', { ...descriptor, value: startMessage });
  installed.add(prototype);
  return Object.freeze({
    capabilities: Object.freeze({
      observe: true, replaceOnNextMessage: true, preload: false,
      refreshCurrent: false, restoreCurrent: false, font: false, fontScale: false,
    }),
    stop() {
      enabled = false;
      const current = Object.getOwnPropertyDescriptor(prototype, 'startMessage');
      if (current && current.value === startMessage) {
        Object.defineProperty(prototype, 'startMessage', descriptor);
        installed.delete(prototype);
      }
    },
  });
}

function validText(text) {
  return typeof text === 'string' && text.length > 0 && text.length <= 16 * 1024;
}

function plainText(text) {
  return !/[\\\u0000-\u0009\u000b-\u001f\u007f]/u.test(text);
}

module.exports = { installMessageAdapter };
