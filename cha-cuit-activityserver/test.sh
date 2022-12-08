# Valid

curl -X POST -H "Content-Type: application/json" -d '{
    "type": "Create",
    "id": "123456",
    "actor": "alice@example.com",
    "object": {
        "type": "Article",
        "name": "My Article",
        "content": "---\ntitle: My Article\nduration: 30 minutes\ntags: [cooking, recipe]\nthumbnail: \"https://example.com/thumbnail.png\"\n---\n\n# Ingrédients\n\n- 1 cup flour\n- 1 tsp baking powder\n- 1/2 tsp salt\n- 1/2 cup milk\n- 1/4 cup vegetable oil\n\n# Équipement\n\n- Mixing bowl\n- Whisk\n- Measuring cups\n- Measuring spoons\n- Baking pan\n\n# Préparation\n\n1. In a mixing bowl, whisk together the flour, baking powder, and salt.\n2. Stir in the milk and vegetable oil until well combined.\n3. Pour the batter into a greased baking pan.\n4. Bake at 350°F for 20-25 minutes, or until a toothpick inserted into the center comes out clean.\n\n# Notes\n\nThis recipe makes one 9x5-inch loaf of bread.",
        "attributedTo": "alice@example.com",
        "mediaType": "text/markdown"
    }
}' http://localhost:8080/inbox


# Invalid
echo "\n"

curl -X POST -H "Content-Type: application/json" -d '{"type": "Create", "id": "123", "actor": "Alice", "object": {"type": "Article", "name": "My article", "content": "# Title 1\n\n- Ingredient 1\n- Ingredient 2\n\n# Title 2\n\n- Equipment 1\n- Equipment 2\n\n# Title 3\n\n1. Step 1\n2. Step 2\n\n# Title 4\n\nNote 1\nNote 2", "attributedTo": "Alice", "mediaType": "text/markdown"}}' http://localhost:8080/inbox


###---
###title: My Article
###duration: 30 minutes
###tags: [cooking, recipe]
###date: yyyy-mm-dd
###thumbnail: "https://example.com/thumbnail.png"
###---
###
#### Ingrédients
###
###- 1 cup flour
###- 1 tsp baking powder
###- 1/2 tsp salt
###- 1/2 cup milk
###- 1/4 cup vegetable oil
###
#### Équipement
###
###- Mixing bowl
###- Whisk
###- Measuring cups
###- Measuring spoons
###- Baking pan
###
#### Préparation
###
###1. In a mixing bowl, whisk together the flour, baking powder, and salt.
###2. Stir in the milk and vegetable oil until well combined.
###3. Pour the batter into a greased baking pan.
###4. Bake at 350°F for 20-25 minutes, or until a toothpick inserted into the center comes out clean.
###
#### Notes
###
###This recipe makes one 9x5-inch loaf of bread.