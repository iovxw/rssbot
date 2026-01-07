#!/usr/bin/env python3
"""Telegram bot script that posts new RSS items to a channel."""

from __future__ import annotations

import argparse
import json
import os
import time
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from html import escape
from typing import Iterable, List, Optional, Tuple

FEED_URLS = [
    "https://rssviewer.app/https%3A%2F%2Frss.app%2Ffeeds%2F_UOnMilxnlEVPTVJz.xml?utm=rssapp",
    "https://rssviewer.app/https%3A%2F%2Frss.app%2Ffeeds%2F_VTIbQ51wkTmYRFJs.xml?utm=rssapp",
    "https://rssviewer.app/https%3A%2F%2Frss.app%2Ffeeds%2F_BPwb1trASrAKKeAE.xml?utm=rssapp"
]
DEFAULT_TOKEN = "8575522536:AAHD7t5kFs4lB26ZCt654Y1GT-JNSHQj1Oo"
DEFAULT_CHANNEL = "@Musk_Grid"
DEFAULT_STATE_PATH = "rss_state.json"
DEFAULT_INTERVAL = 60
MAX_SEEN = 500


@dataclass
class FeedItem:
    item_id: str
    title: str
    link: Optional[str]


def fetch_feed(url: str, timeout: int = 20) -> bytes:
    request = urllib.request.Request(
        url,
        headers={
            "User-Agent": "rssbot-telegram/1.0 (+https://github.com/iovxw/rssbot)",
        },
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read()


def _text(node: Optional[ET.Element]) -> str:
    if node is None or node.text is None:
        return ""
    return node.text.strip()


def parse_rss_items(root: ET.Element) -> List[FeedItem]:
    channel = root.find("channel")
    if channel is None:
        return []
    items: List[FeedItem] = []
    for item in channel.findall("item"):
        title = _text(item.find("title"))
        link = _text(item.find("link")) or None
        guid = _text(item.find("guid"))
        pub_date = _text(item.find("pubDate"))
        item_id = guid or link or f"{title}-{pub_date}".strip()
        if not item_id:
            continue
        items.append(FeedItem(item_id=item_id, title=title or "(no title)", link=link))
    return items


def parse_atom_items(root: ET.Element) -> List[FeedItem]:
    ns = "{http://www.w3.org/2005/Atom}"
    items: List[FeedItem] = []
    for entry in root.findall(f"{ns}entry"):
        title = _text(entry.find(f"{ns}title"))
        entry_id = _text(entry.find(f"{ns}id"))
        link_el = entry.find(f"{ns}link[@rel='alternate']") or entry.find(f"{ns}link")
        link = link_el.get("href") if link_el is not None else None
        item_id = entry_id or link or title
        if not item_id:
            continue
        items.append(FeedItem(item_id=item_id, title=title or "(no title)", link=link))
    return items


def parse_feed(xml_bytes: bytes) -> List[FeedItem]:
    root = ET.fromstring(xml_bytes)
    tag = root.tag.lower()
    if tag.endswith("rss"):
        return parse_rss_items(root)
    if tag.endswith("feed"):
        return parse_atom_items(root)
    return []


def load_state(path: str) -> List[str]:
    if not os.path.exists(path):
        return []
    try:
        with open(path, "r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except (json.JSONDecodeError, OSError):
        return []
    if isinstance(payload, list):
        return [str(item) for item in payload]
    return []


def save_state(path: str, items: Iterable[str]) -> None:
    data = list(items)[-MAX_SEEN:]
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(data, handle, ensure_ascii=False, indent=2)


def build_message(item: FeedItem) -> str:
    title = escape(item.title)
    if item.link:
        return f"<b>{title}</b>\n{escape(item.link)}"
    return f"<b>{title}</b>"


def send_message(token: str, chat_id: str, text: str) -> None:
    endpoint = f"https://api.telegram.org/bot{token}/sendMessage"
    payload = urllib.parse.urlencode(
        {
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML",
            "disable_web_page_preview": False,
        }
    ).encode("utf-8")
    request = urllib.request.Request(endpoint, data=payload)
    with urllib.request.urlopen(request, timeout=20) as response:
        response.read()


def publish_new_items(
    feed_url: str,
    token: str,
    channel: str,
    state_path: str,
) -> Tuple[int, int]:
    seen_ids = load_state(state_path)
    seen_set = set(seen_ids)
    xml_bytes = fetch_feed(feed_url)
    items = parse_feed(xml_bytes)
    new_items = [item for item in items if item.item_id not in seen_set]
    if not new_items:
        return 0, len(seen_set)
    new_items.reverse()
    for item in new_items:
        send_message(token, channel, build_message(item))
        seen_ids.append(item.item_id)
    save_state(state_path, seen_ids)
    return len(new_items), len(seen_ids)


def run_loop(
    feed_url: str,
    token: str,
    channel: str,
    state_path: str,
    interval: int,
    once: bool,
) -> None:
    while True:
        try:
            published, total = publish_new_items(feed_url, token, channel, state_path)
            print(f"Published {published} items. Tracking {total} seen entries.")
        except (urllib.error.URLError, ET.ParseError) as exc:
            print(f"Failed to fetch or parse feed: {exc}")
        except Exception as exc:  # pragma: no cover - defensive
            print(f"Unexpected error: {exc}")
        if once:
            break
        time.sleep(interval)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Post new RSS items to a Telegram channel.",
    )
    parser.add_argument("--feed-url", default=DEFAULT_FEED_URL)
    parser.add_argument("--token", default=DEFAULT_TOKEN)
    parser.add_argument("--channel", default=DEFAULT_CHANNEL)
    parser.add_argument("--state", default=DEFAULT_STATE_PATH)
    parser.add_argument("--interval", type=int, default=DEFAULT_INTERVAL)
    parser.add_argument("--once", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    run_loop(
        feed_url=args.feed_url,
        token=args.token,
        channel=args.channel,
        state_path=args.state,
        interval=args.interval,
        once=args.once,
    )


if __name__ == "__main__":
    main()
