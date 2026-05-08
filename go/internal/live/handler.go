package live

import (
	"database/sql"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/base64"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"log"
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

// MakeToken creates a short-lived HMAC-signed token encoding user/instrument/expiry.
func MakeToken(key []byte, userID, securityID, exchangeSegment string, ttl time.Duration) string {
	payload := base64.RawURLEncoding.EncodeToString([]byte(
		fmt.Sprintf(`{"u":%q,"s":%q,"e":%q,"x":%d}`, userID, securityID, exchangeSegment, time.Now().Add(ttl).Unix()),
	))
	mac := hmac.New(sha256.New, key)
	mac.Write([]byte(payload))
	return payload + "." + hex.EncodeToString(mac.Sum(nil))
}

type tokenPayload struct {
	U string `json:"u"`
	S string `json:"s"`
	E string `json:"e"`
	X int64  `json:"x"`
}

func validateToken(token string, key []byte) (p tokenPayload, ok bool) {
	dot := strings.LastIndexByte(token, '.')
	if dot < 0 {
		return
	}
	payload, sig := token[:dot], token[dot+1:]

	mac := hmac.New(sha256.New, key)
	mac.Write([]byte(payload))
	expected := hex.EncodeToString(mac.Sum(nil))
	if !hmac.Equal([]byte(sig), []byte(expected)) {
		return
	}

	raw, err := base64.RawURLEncoding.DecodeString(payload)
	if err != nil {
		return
	}
	if err := json.Unmarshal(raw, &p); err != nil || time.Now().Unix() > p.X {
		return
	}
	return p, true
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

// Token issues a short-lived signed token for the given instrument.
func (h *Handler) Token(w http.ResponseWriter, r *http.Request) {
	userID := r.Header.Get("X-User-ID")
	if userID == "" {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	securityID := r.URL.Query().Get("security_id")
	exchangeSegment := r.URL.Query().Get("exchange_segment")
	if securityID == "" || exchangeSegment == "" {
		http.Error(w, "missing params", http.StatusBadRequest)
		return
	}
	token := MakeToken(h.EncryptionKey, userID, securityID, exchangeSegment, 30*time.Second)
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{"token": token})
}

// Live upgrades to WebSocket and proxies Dhan quote ticks to the browser.
func (h *Handler) Live(w http.ResponseWriter, r *http.Request) {
	p, ok := validateToken(r.URL.Query().Get("token"), h.EncryptionKey)
	if !ok {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	clientID, accessToken, err := broker.GetToken(h.DB, h.EncryptionKey, p.U)
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

	dhanSeg, _ := candles.MapSegment(p.E)
	dhanURL := fmt.Sprintf("%s?version=2&token=%s&clientId=%s&authType=2", dhanFeedURL, accessToken, clientID)
	dhanConn, _, err := websocket.DefaultDialer.Dial(dhanURL, nil)
	if err != nil {
		log.Printf("live: dhan dial error: %v", err)
		browserConn.WriteMessage(websocket.TextMessage, []byte(`{"error":"feed unavailable"}`))
		return
	}
	defer dhanConn.Close()

	sub, _ := json.Marshal(map[string]any{
		"RequestCode":     17,
		"InstrumentCount": 1,
		"InstrumentList":  []map[string]string{{"ExchangeSegment": dhanSeg, "SecurityId": p.S}},
	})
	if err := dhanConn.WriteMessage(websocket.TextMessage, sub); err != nil {
		return
	}

	// reuse buffer to avoid per-tick allocation
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
	mux.HandleFunc("GET /chart/live/token", h.Token)
	mux.HandleFunc("GET /chart/live", h.Live)
}
