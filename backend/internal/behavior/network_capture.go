package behavior

// NetworkCaptureInjectJS hooks fetch/XHR/WebSocket for complete capture during recording.
const NetworkCaptureInjectJS = `(function(){
  if (window.__ppNetworkCapture) return;
  window.__ppNetworkCapture = { events: [], seq: 0 };
  const cap = window.__ppNetworkCapture;
  const push = (entry) => {
    entry.t = Date.now();
    entry.id = ++cap.seq;
    cap.events.push(entry);
    if (cap.events.length > 2000) cap.events.shift();
  };
  const origFetch = window.fetch;
  if (origFetch) {
    window.fetch = function(input, init) {
      const url = typeof input === 'string' ? input : (input && input.url) || '';
      const method = (init && init.method) || 'GET';
      push({ type: 'fetch', method: method, url: url });
      return origFetch.apply(this, arguments);
    };
  }
  const XHR = window.XMLHttpRequest;
  if (XHR) {
    const open = XHR.prototype.open;
    XHR.prototype.open = function(method, url) {
      this.__ppMethod = method;
      this.__ppUrl = url;
      return open.apply(this, arguments);
    };
    const send = XHR.prototype.send;
    XHR.prototype.send = function() {
      push({ type: 'xhr', method: this.__ppMethod || 'GET', url: this.__ppUrl || '' });
      return send.apply(this, arguments);
    };
  }
  if (window.WebSocket) {
    const OrigWS = window.WebSocket;
    window.WebSocket = function(url, protocols) {
      push({ type: 'websocket', method: 'OPEN', url: String(url || '') });
      return new OrigWS(url, protocols);
    };
    window.WebSocket.prototype = OrigWS.prototype;
  }
})();`

// NetworkRecordedEvent is one captured network interaction.
type NetworkRecordedEvent struct {
	ID     int    `json:"id"`
	T      int64  `json:"t"`
	Type   string `json:"type"`
	Method string `json:"method,omitempty"`
	URL    string `json:"url,omitempty"`
	Status int    `json:"status,omitempty"`
}

const networkCaptureRetrieveJS = `(function(){
  if (!window.__ppNetworkCapture) return [];
  return window.__ppNetworkCapture.events || [];
})();`
