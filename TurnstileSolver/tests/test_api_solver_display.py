from types import SimpleNamespace

from api_solver import TurnstileAPIServer


def test_display_welcome_falls_back_without_rich_for_gbk_console(capsys) -> None:
    server = TurnstileAPIServer(
        headless=True,
        useragent='test-agent',
        debug=False,
        browser_type='chromium',
        thread=1,
        proxy_support=False,
    )

    class FakeConsole:
        def __init__(self) -> None:
            self.file = SimpleNamespace(encoding='gbk')

        def clear(self) -> None:
            return None

        def print(self, *args, **kwargs) -> None:
            raise AssertionError('rich print should not be called for gbk console')

    server.console = FakeConsole()

    server.display_welcome()

    captured = capsys.readouterr()
    assert 'Turnstile Solver' in captured.out
    assert 'QQ: 3779239578' in captured.out
