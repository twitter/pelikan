---
layout: default
title: Blog
permalink: /blog/
---

  <div class="post-list">
    {% for post in site.posts %}
        <span class="post-meta">{{ post.date | date: "%b %-d, %Y" }}</span>

        <h2>
          <a class="post-link" href="{{ post.url | prepend: site.baseurl }}">{{ post.title }}</a>
        </h2>

        {% if post.content.size > 100 %}
           {{ post.content | truncatewords: 50 }}
           <a href="{{ post.url | prepend: site.baseurl }}">read more</a>
        {% else %}
           {{ post.content }}
        {% endif %}
        <hr>

    {% endfor %}

  <div class="page-info" align="right">
    <p class="rss-subscribe">subscribe <a href="{{ "/feed.xml" | prepend: site.baseurl }}">via RSS</a></p>
  <div>
