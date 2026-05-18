import subprocess

def test(html, profile='html'):
    process = subprocess.Popen(
        ['./venv/bin/djlint', '--reformat', f'--profile={profile}', '--max-line-length=120', '-'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    stdout, stderr = process.communicate(input=html)
    print(f"Input: {html.strip()} (profile={profile})")
    print(f"Output:\n{stdout}")
    if stderr:
        print(f"Error:\n{stderr}")
    print("-" * 20)

test('<span><input /></span>')
test('<span><br /></span>')
test('{% if user %}<a title="{{ x }}">T</a>{% endif %}')
test('<div><a title="{{ x }}">T</a></div>')
