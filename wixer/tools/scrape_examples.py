import requests
import os

EXAMPLES_DIR = "examples"
KURTANR_REPO = "kurtanr/WiXInstallerExamples"
BRANCH = "main"

def get_contents(path=""):
    url = f"https://api.github.com/repos/{KURTANR_REPO}/contents/{path}?ref={BRANCH}"
    r = requests.get(url)
    if r.status_code == 200:
        return r.json()
    else:
        print(f"Failed to fetch contents for {path}: {r.status_code}")
        return []

def download_file(url, output_path):
    print(f"Downloading {url} to {output_path}")
    r = requests.get(url)
    if r.status_code == 200:
        os.makedirs(os.path.dirname(output_path), exist_ok=True)
        with open(output_path, 'wb') as f:
            f.write(r.content)
        return True
    else:
        print(f"Failed to download: {r.status_code}")
        return False

def main():
    if not os.path.exists(EXAMPLES_DIR):
        os.makedirs(EXAMPLES_DIR)
    
    base_dir = os.path.join(EXAMPLES_DIR, "kurtanr_WiXInstallerExamples")
    
    # Download License
    download_file(f"https://raw.githubusercontent.com/{KURTANR_REPO}/{BRANCH}/LICENSE", os.path.join(base_dir, "LICENSE"))
    
    contents = get_contents()
    for item in contents:
        if item['type'] == 'dir' and item['name'][0].isdigit():
            # It's an example directory
            example_name = item['name']
            print(f"Processing example: {example_name}")
            
            # Look for Product.wxs or Bundle.wxs inside
            sub_contents = get_contents(example_name)
            for sub in sub_contents:
                if sub['name'].endswith(".wxs"):
                    raw_url = sub['download_url']
                    out_path = os.path.join(base_dir, example_name, sub['name'])
                    download_file(raw_url, out_path)

if __name__ == "__main__":
    main()
