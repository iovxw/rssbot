#!/bin/bash
#
# Debian 8 or higher

[ -w / ] || {
  echo "You are not running as root. Use su - to get the root privilege at first." >&2
  exit 1
}

apt update
apt install bsdtar -y

tag=$(wget -qO- https://api.github.com/repos/iovxw/rssbot/releases/latest | grep 'tag_name' | cut -d\" -f4)

cd /root && wget -qO- "https://github.com/iovxw/rssbot/releases/download/${tag}/rssbot-${tag}-linux.zip" | bsdtar -xvf-
chmod +x /root/rssbot

read -p "Please paste telegram bot token for rssbot here: " token
[ -z "${token}" ]

cat > /lib/systemd/system/rssbot.service<<-EOF
[Service]
ExecStart=/root/rssbot DATAFILE ${token}
EOF

systemctl daemon-reload
systemctl start rssbot
