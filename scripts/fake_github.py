"""A tiny fake GitHub for the release smoke test: the "latest release" API plus the assets.

Usage: fake_github.py PORT VERSION DIR   (serves the files in DIR as release v VERSION)
"""
import http.server, json, os, sys, socketserver
port, version, root = int(sys.argv[1]), sys.argv[2], sys.argv[3]
BASE = f"http://127.0.0.1:{port}"
TAG = f"v{version}"
DL = f"/ecastro-intive/bibu-cli/releases/download/{TAG}"
API = "/api/v3/repos/ecastro-intive/bibu-cli/releases"
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def _json(self, obj):
        data = json.dumps(obj).encode(); self.send_response(200)
        self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(data))); self.end_headers(); self.wfile.write(data)
    def release(self):
        assets = [{"name": n, "url": f"{BASE}{DL}/{n}", "browser_download_url": f"{BASE}{DL}/{n}"}
                  for n in sorted(os.listdir(root)) if os.path.isfile(os.path.join(root, n))]
        return {"tag_name": TAG, "name": TAG, "url": f"{BASE}{API}/1", "assets": assets, "prerelease": False}
    def do_GET(self):
        p = self.path.split("?")[0]
        sys.stderr.write(f"GET {p}\n"); sys.stderr.flush()
        if p in (f"{API}/latest", f"{API}/tags/{TAG}"): return self._json(self.release())
        if p == API: return self._json([self.release()])
        if p.startswith(DL + "/"):
            f = os.path.join(root, p[len(DL) + 1:])
            if os.path.isfile(f):
                data = open(f, "rb").read(); self.send_response(200)
                self.send_header("Content-Length", str(len(data))); self.end_headers(); self.wfile.write(data); return
        self.send_response(404); self.end_headers()
socketserver.TCPServer.allow_reuse_address = True
with socketserver.TCPServer(("127.0.0.1", port), H) as s: s.serve_forever()
