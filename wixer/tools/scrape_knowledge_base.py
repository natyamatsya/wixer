import requests
from bs4 import BeautifulSoup
import json
import os
import re

# URLs to scrape
MSI_PROPS_URL = "https://learn.microsoft.com/en-us/windows/win32/msi/property-reference"

def scrape_msi_properties():
    print(f"Scraping {MSI_PROPS_URL}...")
    response = requests.get(MSI_PROPS_URL)
    response.raise_for_status()
    
    soup = BeautifulSoup(response.text, 'html.parser')
    
    properties = []
    
    # Microsoft docs usually put this in a table
    tables = soup.find_all('table')
    
    for table in tables:
        rows = table.find_all('tr')
        for row in rows:
            cols = row.find_all('td')
            if len(cols) >= 2:
                name_tag = cols[0].find('a')
                if name_tag:
                    name = name_tag.text.strip()
                    link = name_tag['href']
                    if not link.startswith('http'):
                        link = "https://learn.microsoft.com/en-us/windows/win32/msi/" + link
                else:
                    name = cols[0].text.strip()
                    link = None
                
                description = cols[1].text.strip()
                
                # Filter out non-properties
                if " " not in name and name.isupper() or name[0].isupper():
                    tags = ["MSI"]
                    
                    # heuristic for version
                    # e.g. "Available starting with Windows Installer 4.0."
                    version_match = re.search(r"Windows Installer (\d+\.\d+)", description)
                    if version_match:
                        tags.append(f"MSI {version_match.group(1)}")
                    
                    properties.append({
                        "name": name,
                        "kind": "Property",
                        "description": description,
                        "docs_url": link,
                        "tags": tags
                    })
                    
    print(f"Found {len(properties)} properties.")
    return properties

def main():
    base_props = scrape_msi_properties()
    
    # Add manual entries
    manual_entries = [
         {
            "name": "INSTALLFOLDER",
            "kind": "Directory",
            "description": "The default identifier for the main installation directory.",
            "docs_url": None,
            "tags": ["Directory", "Standard"]
        },
        {
            "name": "ProgramFilesFolder",
            "kind": "Directory",
            "description": "The Program Files folder (32-bit on 32-bit systems, or 32-bit on 64-bit systems).",
            "docs_url": "https://learn.microsoft.com/en-us/windows/win32/msi/programfilesfolder",
            "tags": ["Directory", "Standard", "MSI"]
        },
        {
            "name": "CPACK_WIX_PRODUCT_GUID",
            "kind": "CPack",
            "description": "The ProductCode GUID generated or specified by CPack.",
            "docs_url": "https://cmake.org/cmake/help/latest/cpack_gen/wix.html#variable:CPACK_WIX_PRODUCT_GUID",
            "tags": ["CPack", "CMake"]
        },
        {
            "name": "CPACK_WIX_UPGRADE_GUID",
            "kind": "CPack",
            "description": "The UpgradeCode GUID generated or specified by CPack.",
            "docs_url": "https://cmake.org/cmake/help/latest/cpack_gen/wix.html#variable:CPACK_WIX_UPGRADE_GUID",
            "tags": ["CPack", "CMake"]
        }
    ]
    
    # Merge manual entries with scraped ones (manual overrides if needed, but here we append/dedup)
    # Let's use a dict to dedup by name
    prop_map = {p['name']: p for p in base_props}
    for p in manual_entries:
        prop_map[p['name']] = p
        
    all_props = list(prop_map.values())
    
    # Sort by name
    all_props.sort(key=lambda x: x['name'])
    
    output_path = os.path.join(os.path.dirname(__file__), "../assets/knowledge_base.json")
    output_path = os.path.abspath(output_path)
    
    with open(output_path, 'w') as f:
        json.dump(all_props, f, indent=2)
        
    print(f"Updated {output_path}")
