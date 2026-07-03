"""Chrome DevTools Protocol driver for the Overnotes WebView2.

Launch overnotes with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222
then use this script to send deterministic input events and evaluate JS.

Usage:
  python cdp.py click 100 200
  python cdp.py dblclick 100 200
  python cdp.py drag 100 200 300 400
  python cdp.py wheel 500 400 -- -120
  python cdp.py type "hello world"
  python cdp.py key Enter
  python cdp.py eval "document.title"
  python cdp.py shot out.png
"""
import json
import sys
import time
import urllib.request

import websocket

PORT = 9222


def get_pages(port=PORT):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/json") as r:
        pages = json.load(r)
    pages = [p for p in pages if p.get("type") == "page"]
    if not pages:
        raise RuntimeError("no debuggable pages found")
    return pages


class Cdp:
    def __init__(self, port=PORT, ws_url=None):
        if ws_url is None:
            ws_url = get_pages(port)[0]["webSocketDebuggerUrl"]
        self.ws = websocket.create_connection(ws_url, timeout=10)
        self.next_id = 1

    @classmethod
    def for_selector(cls, css, port=PORT):
        """Connect to the first page whose DOM matches a CSS selector."""
        for page in get_pages(port):
            c = cls(ws_url=page["webSocketDebuggerUrl"])
            try:
                if c.eval(f"!!document.querySelector({json.dumps(css)})"):
                    return c
            except RuntimeError:
                pass
            c.ws.close()
        raise RuntimeError(f"no page with element {css!r}")

    def call(self, method, **params):
        mid = self.next_id
        self.next_id += 1
        self.ws.send(json.dumps({"id": mid, "method": method, "params": params}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == mid:
                if "error" in msg:
                    raise RuntimeError(f"{method}: {msg['error']}")
                return msg.get("result", {})

    def mouse(self, kind, x, y, button="none", clicks=0):
        self.call(
            "Input.dispatchMouseEvent",
            type=kind, x=float(x), y=float(y),
            button=button, clickCount=clicks,
            buttons=1 if button == "left" and kind != "mouseReleased" else 0,
        )

    def click(self, x, y, clicks=1):
        self.mouse("mouseMoved", x, y)
        time.sleep(0.05)
        self.mouse("mousePressed", x, y, "left", clicks)
        time.sleep(0.05)
        self.mouse("mouseReleased", x, y, "left", clicks)

    def drag(self, x1, y1, x2, y2, steps=12):
        self.mouse("mouseMoved", x1, y1)
        time.sleep(0.05)
        self.mouse("mousePressed", x1, y1, "left", 1)
        time.sleep(0.06)
        for i in range(1, steps + 1):
            x = x1 + (x2 - x1) * i / steps
            y = y1 + (y2 - y1) * i / steps
            self.mouse("mouseMoved", x, y)
            time.sleep(0.02)
        time.sleep(0.06)
        self.mouse("mouseReleased", x2, y2, "left", 1)

    def wheel(self, x, y, delta_y):
        self.call(
            "Input.dispatchMouseEvent",
            type="mouseWheel", x=float(x), y=float(y),
            deltaX=0.0, deltaY=float(delta_y), button="none", clickCount=0,
        )

    def type_text(self, text):
        for ch in text:
            self.call("Input.dispatchKeyEvent", type="char", text=ch)
            time.sleep(0.01)

    def key(self, key_name):
        keymap = {
            "Enter": ("Enter", "Enter", 13),
            "Escape": ("Escape", "Escape", 27),
            "Delete": ("Delete", "Delete", 46),
            "Backspace": ("Backspace", "Backspace", 8),
            "Tab": ("Tab", "Tab", 9),
        }
        key, code, vk = keymap.get(key_name, (key_name, key_name, 0))
        self.call("Input.dispatchKeyEvent", type="keyDown", key=key, code=code,
                  windowsVirtualKeyCode=vk, nativeVirtualKeyCode=vk)
        time.sleep(0.03)
        self.call("Input.dispatchKeyEvent", type="keyUp", key=key, code=code,
                  windowsVirtualKeyCode=vk, nativeVirtualKeyCode=vk)

    def eval(self, expr):
        r = self.call("Runtime.evaluate", expression=expr, returnByValue=True)
        return r.get("result", {}).get("value")

    def screenshot(self, path):
        import base64
        r = self.call("Page.captureScreenshot", format="png")
        with open(path, "wb") as f:
            f.write(base64.b64decode(r["data"]))


def main():
    args = [a for a in sys.argv[1:] if a != "--"]
    # `--page <css>` selects the window whose DOM matches the selector.
    page_sel = None
    if args[0] == "--page":
        page_sel = args[1]
        args = args[2:]
    cmd = args[0]
    c = Cdp.for_selector(page_sel) if page_sel else Cdp()
    if cmd == "click":
        c.click(float(args[1]), float(args[2]))
        print(f"clicked {args[1]},{args[2]}")
    elif cmd == "dblclick":
        c.click(float(args[1]), float(args[2]))
        time.sleep(0.05)
        c.click(float(args[1]), float(args[2]), clicks=2)
        print(f"double-clicked {args[1]},{args[2]}")
    elif cmd == "drag":
        c.drag(*(float(a) for a in args[1:5]))
        print(f"dragged {args[1:5]}")
    elif cmd == "wheel":
        c.wheel(float(args[1]), float(args[2]), float(args[3]))
        print(f"wheel {args[3]} at {args[1]},{args[2]}")
    elif cmd == "type":
        c.type_text(args[1])
        print("typed")
    elif cmd == "key":
        c.key(args[1])
        print(f"key {args[1]}")
    elif cmd == "eval":
        print(json.dumps(c.eval(args[1])))
    elif cmd == "clicksel":
        # click the center of the first element matching a CSS selector
        rect = c.eval(
            "(() => { const e = document.querySelector(" + json.dumps(args[1]) + ");"
            " if (!e) return null; const r = e.getBoundingClientRect();"
            " return [r.x + r.width / 2, r.y + r.height / 2]; })()"
        )
        if rect is None:
            print(f"no element for {args[1]}", file=sys.stderr)
            sys.exit(1)
        c.click(rect[0], rect[1])
        print(f"clicked {args[1]} at {rect[0]:.0f},{rect[1]:.0f}")
    elif cmd == "rect":
        print(json.dumps(c.eval(
            "(() => { const e = document.querySelector(" + json.dumps(args[1]) + ");"
            " if (!e) return null; const r = e.getBoundingClientRect();"
            " return {x: r.x, y: r.y, w: r.width, h: r.height}; })()"
        )))
    elif cmd == "shot":
        c.screenshot(args[1])
        print(f"saved {args[1]}")
    else:
        print(f"unknown command {cmd}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
