FROM centos:7

RUN yum -y update
RUN yum -y install unzip

WORKDIR /app

RUN curl -LO 'https://github.com/iovxw/rssbot/releases/download/v1.4.4/rssbot-v1.4.4-linux.zip'

RUN unzip rssbot-v1.4.4-linux.zip

CMD /app/rssbot $TELEGRAM_DATAFILE $TELEGRAM_BOT_TOKEN