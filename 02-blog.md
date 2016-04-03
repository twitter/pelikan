---
layout: default
title: Blog
permalink: /blog/
---
  <div class="page-info">
    <p>We plan to release a series of posts in the upcoming weeks. Please take a
    look at our <a href="https://github.com/twitter/pelikan/wiki/Technical-details">
    topics</a> and let us know what (else) interests you.</p>
  </div>

  <ul class="post-list">
    {% for post in site.posts %}
      <li>
        <span class="post-meta">{{ post.date | date: "%b %-d, %Y" }}</span>

        <h2>
          <a class="post-link" href="{{ post.url | prepend: site.baseurl }}">{{ post.title }}</a>
        </h2>
        {% if post.content.size > 300 %}
           {{ post.content | truncatewords: 150 }}
           <a href="{{ post.url | prepend: site.baseurl }}">read more</a>
        {% else %}
           {{ post.content }}
        {% endif %}
      </li>
    {% endfor %}
  </ul>

  <div class="page-info">
    <p class="rss-subscribe">subscribe <a href="{{ "/feed.xml" | prepend: site.baseurl }}">via RSS</a></p>
  <div>
