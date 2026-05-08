package live

import (
	"crypto/hmac"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/gorilla/websocket"
	"github.com/madhav165/dhan-test/go/internal/broker"
	"github.com/madhav165/dhan-test/go/internal/candles"
)

const (
	dhanFeedURL    = "wss://api-feed.dhan.co"
	quotePacketLen = 50
	quoteType      = byte(4)
)

type Handler struct {
	DB            *sql.DB
	EncryptionKey []byte
	upgrader      websocket.Upgrader
}

func NewHandler(db *sql.DB, key []byte) *Handler {
	return &Handler{
		DB:            db,
		EncryptionKey: key,
		upgrader:      websocket.Upgrader{CheckOrigin: func(r *http.Request) bool { return true }},
	}
}

// MakeToken creates an HMAC-signed token encoding userID and expiry.
// Called from SvelteKit's page load (server-side) to pre-generate the token.
func MakeToken(key []byte, userID string, ttl time.Duration) string {
	payload := base64.RawURLEncoding.EncodeToString([]byte(
		fmt.Sprintf(`{"u":%q,"x":%d}`, userID, time.Now().Add(ttl).Unix()),
	))
	mac := hmac.New(sha256.New, key)
	mac.Write([]byte(payload))
	return payload + "." + hex.EncodeToString(mac.Sum(nil))
}

type tokenPayload struct {
	U string `json:"u"`
	X int64  `json:"x"`
}

func validateToken(token string, key []byte) (userID string, ok bool) {
	dot := strings.LastIndexByte(token, '.')
	if dot < 0 {
		return
	}
	payload, sig := token[:dot], token[dot+1:]

	mac := hmac.New(sha256.New, key)
	mac.Write([]byte(payload))
	if !hmac.Equal([]byte(hex.EncodeToString(mac.Sum(nil))), []byte(sig)) {
		return
	}

	raw, err := base64.RawURLEncoding.DecodeString(payload)
	if err != nil {
		return
	}
	var p tokenPayload
	if err := json.Unmarshal(raw, &p); err != nil || time.Now().Unix() > p.X {
		return
	}
	return p.U, true
}

// parseQuote extracts LTP, LTT, Volume from a Dhan binary quote packet (50 bytes, type=4).
// Binary layout (little-endian): B H B I f H I f I I I f f f f
// offsets:                        0 1 3 4 8 12 14 18 22 26 30 34 38 42 46
func parseQuote(b []byte) (ltp float32, ltt uint32, vol uint32, ok bool) {
	if len(b) < quotePacketLen || b[0] != quoteType {
		return
	}
	ltp = math.Float32frombits(binary.LittleEndian.Uint32(b[8:]))
	ltt = binary.LittleEndian.Uint32(b[14:])
	vol = binary.LittleEndian.Uint32(b[22:])
	return ltp, ltt, vol, true
}

// Live upgrades to WebSocket and proxies Dhan quote ticks to the browser.
func (h *Handler) Live(w http.ResponseWriter, r *http.Request) {
	userID, ok := validateToken(r.URL.Query().Get("token"), h.EncryptionKey)
	if !ok {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	securityID := r.URL.Query().Get("security_id")
	exchangeSegment := r.URL.Query().Get("exchange_segment")
	if securityID == "" || exchangeSegment == "" {
		http.Error(w, "missing params", http.StatusBadRequest)
		return
	}

	clientID, accessToken, err := broker.GetToken(h.DB, h.EncryptionKey, userID)
	if err == sql.ErrNoRows {
		http.Error(w, "broker not connected", http.StatusUnauthorized)
		return
	}
	if err != nil {
		http.Error(w, "broker error", http.StatusInternalServerError)
		return
	}

	browserConn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		return
	}
	defer browserConn.Close()

	dhanSeg, _ := candles.MapSegment(exchangeSegment)
	dhanURL := fmt.Sprintf("%s?version=2&token=%s&clientId=%s&authType=2", dhanFeedURL, accessToken, clientID)
	dhanConn, _, err := websocket.DefaultDialer.Dial(dhanURL, nil)
	if err != nil {
		browserConn.WriteMessage(websocket.TextMessage, []byte(`{"error":"feed unavailable"}`))
		return
	}
	defer dhanConn.Close()

	sub, _ := json.Marshal(map[string]any{
		"RequestCode":     17,
		"InstrumentCount": 1,
		"InstrumentList":  []map[string]string{{"ExchangeSegment": dhanSeg, "SecurityId": securityID}},
	})
	if err := dhanConn.WriteMessage(websocket.TextMessage, sub); err != nil {
		return
	}

	buf := make([]byte, 0, 64)
	for {
		_, msg, err := dhanConn.ReadMessage()
		if err != nil {
			return
		}
		ltp, ltt, vol, ok := parseQuote(msg)
		if !ok {
			continue
		}
		buf = buf[:0]
		buf = append(buf, `{"ltp":`...)
		buf = strconv.AppendFloat(buf, float64(ltp), 'f', 2, 32)
		buf = append(buf, `,"ltt":`...)
		buf = strconv.AppendUint(buf, uint64(ltt), 10)
		buf = append(buf, `,"vol":`...)
		buf = strconv.AppendUint(buf, uint64(vol), 10)
		buf = append(buf, '}')
		if err := browserConn.WriteMessage(websocket.TextMessage, buf); err != nil {
			return
		}
	}
}

func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /chart/live", h.Live)
}
