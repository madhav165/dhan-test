package telegram

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"math/rand"
	"net/http"
	"strings"
	"time"
)

type Bot struct {
	Token string
	DB    *sql.DB
}

type tgUpdate struct {
	UpdateID int `json:"update_id"`
	Message  *struct {
		Text string `json:"text"`
		Chat struct {
			ID int64 `json:"id"`
		} `json:"chat"`
	} `json:"message"`
}

type tgUpdatesResp struct {
	OK     bool       `json:"ok"`
	Result []tgUpdate `json:"result"`
}

func (b *Bot) SendMessage(chatID string, text string) {
	url := fmt.Sprintf("https://api.telegram.org/bot%s/sendMessage", b.Token)
	body := fmt.Sprintf(`{"chat_id":%s,"text":%q}`, chatID, text)
	resp, err := http.Post(url, "application/json", strings.NewReader(body))
	if err != nil {
		log.Printf("telegram sendMessage: %v", err)
		return
	}
	resp.Body.Close()
}

func (b *Bot) PollForever() {
	offset := 0
	client := &http.Client{Timeout: 35 * time.Second}

	for {
		url := fmt.Sprintf(
			"https://api.telegram.org/bot%s/getUpdates?timeout=30&offset=%d",
			b.Token, offset,
		)
		resp, err := client.Get(url)
		if err != nil {
			time.Sleep(2 * time.Second)
			continue
		}

		var updates tgUpdatesResp
		json.NewDecoder(resp.Body).Decode(&updates)
		resp.Body.Close()

		for _, u := range updates.Result {
			offset = u.UpdateID + 1
			if u.Message == nil || !strings.HasPrefix(u.Message.Text, "/start") {
				continue
			}
			chatID := fmt.Sprintf("%d", u.Message.Chat.ID)
			b.handleStart(chatID)
		}
	}
}

func (b *Bot) handleStart(chatID string) {
	otp := fmt.Sprintf("%06d", rand.Intn(1000000))
	_, err := b.DB.Exec(
		`insert into telegram_link_tokens (token, chat_id, expires_at)
		 values ($1, $2, now() + interval '10 minutes')
		 on conflict (token) do update set chat_id = excluded.chat_id, expires_at = excluded.expires_at`,
		otp, chatID,
	)
	if err != nil {
		log.Printf("telegram handleStart: %v", err)
		return
	}
	b.SendMessage(chatID, fmt.Sprintf("Your connection code is: %s\n(expires in 10 minutes)", otp))
}
