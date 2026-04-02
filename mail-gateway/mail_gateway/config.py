from dataclasses import dataclass
import os


@dataclass(frozen=True)
class Settings:
    host: str
    port: int
    database_path: str
    luckmail_base_url: str
    luckmail_api_key: str

    def provider_statuses(self) -> dict[str, str]:
        return {
            "luckmail": "enabled" if self.luckmail_api_key else "disabled",
            "yyds_mail": "disabled",
            "duckmail": "disabled",
        }


def load_settings() -> Settings:
    return Settings(
        host=os.getenv("MAIL_GATEWAY_HOST", "127.0.0.1"),
        port=int(os.getenv("MAIL_GATEWAY_PORT", "8081")),
        database_path=os.getenv("MAIL_GATEWAY_DB", "./data/mail_gateway.db"),
        luckmail_base_url=os.getenv("LUCKMAIL_BASE_URL", "https://mails.luckyous.com"),
        luckmail_api_key=os.getenv("LUCKMAIL_API_KEY", ""),
    )
