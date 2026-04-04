from dataclasses import dataclass
import os


@dataclass(frozen=True)
class Settings:
    host: str
    port: int
    database_path: str
    luckmail_base_url: str
    luckmail_api_key: str
    yyds_base_url: str
    yyds_api_key: str
    mail_chatgpt_uk_base_url: str
    mail_chatgpt_uk_api_key: str

    def provider_statuses(self) -> dict[str, str]:
        return {
            'luckmail': 'enabled' if self.luckmail_api_key else 'disabled',
            'yyds_mail': 'enabled' if self.yyds_api_key else 'disabled',
            'mail_chatgpt_uk': 'enabled' if self.mail_chatgpt_uk_api_key else 'disabled',
            'duckmail': 'disabled',
        }


def load_settings() -> Settings:
    return Settings(
        host=os.getenv('MAIL_GATEWAY_HOST', '127.0.0.1'),
        port=int(os.getenv('MAIL_GATEWAY_PORT', '8081')),
        database_path=os.getenv('MAIL_GATEWAY_DB', './data/mail_gateway.db'),
        luckmail_base_url=os.getenv('LUCKMAIL_BASE_URL', 'https://mails.luckyous.com'),
        luckmail_api_key=os.getenv('LUCKMAIL_API_KEY', ''),
        yyds_base_url=os.getenv('YYDS_BASE_URL', 'https://maliapi.215.im/v1'),
        yyds_api_key=os.getenv('YYDS_API_KEY', ''),
        mail_chatgpt_uk_base_url=os.getenv('MAIL_CHATGPT_UK_BASE_URL', 'https://mail.chatgpt.org.uk'),
        mail_chatgpt_uk_api_key=os.getenv('MAIL_CHATGPT_UK_API_KEY', ''),
    )
